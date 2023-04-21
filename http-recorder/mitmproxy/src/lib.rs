#![feature(iterator_try_collect)]

use anyhow::Context;
use http_recorder::{header, request, response};
use pyo3::{pyclass, pymethods, pymodule, FromPyObject};
use std::{
    fs, io,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, RwLock},
};

#[derive(FromPyObject)]
struct Headers<'a> {
    fields: Vec<(&'a [u8], &'a [u8])>,
}
impl<'a> Headers<'a> {
    fn to_headers(&self) -> anyhow::Result<http_recorder::Headers> {
        self.fields
            .iter()
            .map(|(k, v)| -> anyhow::Result<http_recorder::Header> {
                Ok(http_recorder::Header {
                    name: header::HeaderName::from_lower(
                        http::HeaderName::from_bytes(*k)
                            .context("failed to parse header name")?
                            .as_str(),
                    ),
                    value: {
                        let v = http::HeaderValue::from_bytes(*v)
                            .context("failed to parse header value")?;
                        match v.to_str() {
                            Ok(s) => header::HeaderValue::Text(s.to_string()),
                            Err(_) => header::HeaderValue::Binary(
                                v.as_bytes().to_owned().into_boxed_slice(),
                            ),
                        }
                    },
                })
            })
            .try_collect()
    }
}

fn get_content_type(
    headers: &http_recorder::Headers,
    url: &str,
    content: &[u8],
) -> anyhow::Result<mime::Mime> {
    use mime_sniffer::MimeTypeSnifferExt;
    Ok(
        match headers.iter().find(|v| v.name == header::CONTENT_TYPE) {
            Some(v) => match &v.value {
                header::HeaderValue::Text(t) => mime_sniffer::HttpRequest {
                    url: &url,
                    content: &content,
                    type_hint: t,
                }
                .sniff_mime_type_ext(),
                header::HeaderValue::Binary(_) => {
                    anyhow::bail!("invalid Content-Type value: binary data")
                }
            },
            None => mime_sniffer::HttpRequest {
                url: &url,
                content: &content,
                type_hint: mime::APPLICATION_OCTET_STREAM.as_ref(),
            }
            .sniff_mime_type_ext(),
        }
        .unwrap_or(mime::APPLICATION_OCTET_STREAM),
    )
}
fn to_content(content_type: mime::Mime, content: &[u8]) -> http_recorder::Content {
    use sha2::{digest::FixedOutput, Digest, Sha256};
    http_recorder::Content {
        digest: {
            http_recorder::Digest::SHA256(http_recorder::Bytes(
                Sha256::new_with_prefix(content).finalize_fixed().into(),
            ))
        },
        extension: mime2ext::mime2ext(&content_type).map(|v| v.to_string()),
        content_type,
        data: Some(content.to_owned().into_boxed_slice()),
    }
}

#[derive(FromPyObject)]
struct Request<'a> {
    timestamp_start: f64,
    http_version: &'a str,
    method: &'a str,
    url: &'a str,
    headers: Headers<'a>,
    cookies: Vec<(&'a str, &'a str)>,
    content: Option<&'a [u8]>,
}
impl<'a> Request<'a> {
    fn to_request(&self) -> anyhow::Result<request::Request> {
        let headers = self.headers.to_headers()?;
        Ok(request::Request {
            http_version: self
                .http_version
                .parse()
                .context("failed to parse request http version")?,
            method: self.method.parse().unwrap(),
            url: {
                let u = url::Url::parse(self.url)
                    .with_context(|| format!("failed to parse request url: {}", self.url))?;
                request::Url {
                    url_text: self.url.to_owned(),
                    scheme: u.scheme().to_owned(),
                    host: u.host().map(|h| match h {
                        url::Host::Domain(d) => request::Host::Domain(d.to_owned()),
                        url::Host::Ipv4(a) => request::Host::Addr(std::net::IpAddr::V4(a)),
                        url::Host::Ipv6(a) => request::Host::Addr(std::net::IpAddr::V6(a)),
                    }),
                    port: u.port(),
                    path: u.path().to_owned(),
                    query: u
                        .query_pairs()
                        .map(|(k, v)| http_recorder::QueryString {
                            name: k.to_string(),
                            value: v.to_string(),
                        })
                        .collect(),
                }
            },
            cookies: self
                .cookies
                .iter()
                .map(|(k, v)| request::Cookie {
                    name: k.to_string(),
                    value: v.to_string(),
                })
                .collect(),
            body: match self.content {
                Some([]) => None,
                Some(content) => {
                    let content_type = get_content_type(&headers, self.url, content)?;
                    Some(if content_type == mime::APPLICATION_WWW_FORM_URLENCODED {
                        request::Body::UrlEncodedForm(
                            url::form_urlencoded::parse(content)
                                .map(|(k, v)| request::UrlEncodedFormEntry {
                                    name: k.to_string(),
                                    value: v.to_string(),
                                })
                                .collect(),
                        )
                    } else if content_type == mime::MULTIPART_FORM_DATA {
                        let mut parser = multer::Multipart::new::<_, _, std::convert::Infallible, _>(
                            futures_util::stream::once(std::future::ready(Ok(
                                bytes::Bytes::copy_from_slice(content),
                            ))),
                            multer::parse_boundary(content_type)
                                .context("failed to parse request multipart body boundary")?,
                        );
                        let mut ret = Vec::new();
                        while let Some(f) = futures::executor::block_on(parser.next_field())
                            .context("failed to get multipart form field")?
                        {
                            ret.push(request::MultipartFormEntry {
                                name: f.name().map(str::to_string),
                                file_name: f.file_name().map(str::to_string),
                                headers: f
                                    .headers()
                                    .iter()
                                    .map(|(k, v)| http_recorder::Header {
                                        name: header::HeaderName::from_lower(k.as_str()),
                                        value: match v.to_str() {
                                            Ok(s) => header::HeaderValue::Text(s.to_string()),
                                            Err(_) => header::HeaderValue::Binary(
                                                v.as_bytes().to_vec().into_boxed_slice(),
                                            ),
                                        },
                                    })
                                    .collect(),
                                content: {
                                    use mime_sniffer::MimeTypeSnifferExt;
                                    let content_type = f.content_type().cloned();
                                    let content = futures::executor::block_on(f.bytes())
                                        .context("failed to parse multipart body bytes")?;
                                    to_content(
                                        match content_type {
                                            Some(ct) => mime_sniffer::HttpRequest {
                                                url: &self.url,
                                                content: &content,
                                                type_hint: ct.as_ref(),
                                            }
                                            .sniff_mime_type_ext(),
                                            None => mime_sniffer::HttpRequest {
                                                url: &self.url,
                                                content: &content,
                                                type_hint: mime::APPLICATION_OCTET_STREAM.as_ref(),
                                            }
                                            .sniff_mime_type_ext(),
                                        }
                                        .unwrap_or(mime::APPLICATION_OCTET_STREAM),
                                        content.as_ref(),
                                    )
                                },
                            })
                        }
                        request::Body::MultipartForm(ret)
                    } else {
                        request::Body::Content(to_content(content_type, content))
                    })
                }
                None => None,
            },
            headers,
        })
    }
}

#[derive(FromPyObject)]
struct Response<'a> {
    timestamp_end: f64,
    http_version: &'a str,
    status_code: u16,
    headers: Headers<'a>,
    content: Option<&'a [u8]>,
}
impl<'a> Response<'a> {
    fn to_response(&self, url: &str) -> anyhow::Result<response::Response> {
        let headers = self.headers.to_headers()?;
        Ok(response::Response {
            http_version: self
                .http_version
                .parse()
                .context("failed to parse response http version")?,
            status: http_recorder::StatusCode(self.status_code),
            cookies: headers
                .iter()
                .filter(|v| v.name == header::SET_COOKIE)
                .map(|v| {
                    let cok = cookie::Cookie::parse_encoded(
                        if let header::HeaderValue::Text(s) = &v.value {
                            s.as_str()
                        } else {
                            anyhow::bail!("invalid Set-Cookie header: binary value")
                        },
                    )
                    .context("failed to parse response cookie")?;
                    Ok(response::Cookie {
                        name: cok.name().to_owned(),
                        value: cok.value().to_owned(),
                        domain: cok.domain().map(str::to_string),
                        path: cok.path().map(str::to_string),
                        http_only: cok.http_only(),
                        secure: cok.secure(),
                        same_site: cok.same_site().map(|v| match v {
                            cookie::SameSite::Lax => response::SameSite::Lax,
                            cookie::SameSite::None => response::SameSite::None,
                            cookie::SameSite::Strict => response::SameSite::Strict,
                        }),
                        max_age: match cok.max_age() {
                            Some(d) => {
                                Some(std::time::Duration::try_from(d).context("invalud max age")?)
                            }
                            None => None,
                        },
                        expires: cok.expires().map(|v| match v {
                            cookie::Expiration::DateTime(d) => response::Expiration::DateTime({
                                use chrono::TimeZone;
                                let u = chrono::Utc;
                                u.timestamp_nanos(d.unix_timestamp_nanos() as i64)
                            }),
                            cookie::Expiration::Session => response::Expiration::Session,
                        }),
                    })
                })
                .try_collect()?,
            content: match self.content {
                Some([]) => None,
                Some(content) => Some(to_content(
                    get_content_type(&headers, url, content)?,
                    content,
                )),
                None => None,
            },
            headers,
        })
    }
}

#[derive(FromPyObject)]
enum Addr<'a> {
    Short((&'a str, u16)),
    Long((&'a str, u16, &'a pyo3::PyAny, &'a pyo3::PyAny)),
}
impl<'a> Addr<'a> {
    fn to_addr(&self) -> anyhow::Result<std::net::SocketAddr> {
        let (a, p) = match self {
            Self::Short(ap) => *ap,
            Self::Long((a, p, _, _)) => (*a, *p),
        };
        Ok(std::net::SocketAddr::new(
            a.parse().context("failed to parse address")?,
            p,
        ))
    }
}

#[derive(FromPyObject)]
struct Client<'a> {
    peername: Addr<'a>,
}
#[derive(FromPyObject)]
struct Server<'a> {
    peername: Option<Addr<'a>>,
}
#[derive(FromPyObject)]
pub struct Flow<'a> {
    client_conn: Client<'a>,
    server_conn: Server<'a>,
    request: Request<'a>,
    response: Response<'a>,
}
impl<'a> Flow<'a> {
    fn to_entry(self) -> anyhow::Result<http_recorder::Entry> {
        use chrono::TimeZone;
        let utc = chrono::Utc;
        Ok(http_recorder::Entry {
            client_addr: self.client_conn.peername.to_addr()?,
            server_addr: match self.server_conn.peername {
                Some(a) => Some(a.to_addr()?),
                None => None,
            },
            timings: http_recorder::Timings {
                start_time: utc
                    .timestamp_nanos((self.request.timestamp_start * 1_000_000_000f64) as i64),
                finish_time: utc
                    .timestamp_nanos((self.response.timestamp_end * 1000_000_000f64) as i64),
            },
            response: self.response.to_response(self.request.url)?,
            request: self.request.to_request()?,
        })
    }
}

type Data = Arc<RwLock<http_recorder::HttpRecord>>;
type Cancal = Arc<(Mutex<bool>, Condvar)>;

pub struct Saver {
    cancal: Cancal,
    count: u32,
    data: Data,
    duration: std::time::Duration,
    tmp_dest: PathBuf,
    old_path: PathBuf,
    active_path: PathBuf,
    active_file: fs::File,
    old_file: fs::File,
}
impl Saver {
    fn new(cancal: Cancal, data: Data, duration: std::time::Duration) -> anyhow::Result<Self> {
        let tmp_dir = tempfile::Builder::new()
            .prefix("http-recorder-mitmproxy")
            .tempdir()
            .context("failed to create temp directory")?;
        log::info!("temporary data dir: {}", tmp_dir.path().display());
        let old_path = tmp_dir.path().join("init0");
        let active_path = tmp_dir.path().join("init1");
        Ok(Self {
            cancal,
            count: 0,
            data,
            duration,
            active_file: fs::File::create(&active_path).context("failed to create tmp file")?,
            old_file: fs::File::create(&old_path).context("failed to create tmp file")?,
            tmp_dest: tmp_dir.into_path(),
            old_path,
            active_path,
        })
    }
    fn save_data(&mut self) -> anyhow::Result<()> {
        use std::{
            io::{Seek, Write},
            mem::swap,
            ops::Deref,
        };
        if self
            .old_file
            .stream_position()
            .context("failed to get position")?
            != 0
        {
            self.old_file.rewind().context("failed to seek to begin")?;
            self.old_file
                .set_len(0)
                .context("failed to set file length")?;
        }
        {
            let mut buf = xz2::write::XzEncoder::new(io::BufWriter::new(&self.old_file), 3);
            let lg = self.data.read().unwrap();
            ciborium::ser::into_writer(lg.deref(), &mut buf).context("failed to write file")?;
            buf.finish()
                .context("failed to finish compression")?
                .flush()
                .context("failed to flush buffer")?;
        }

        {
            let mut new_path = self.tmp_dest.join(format!("{}.bin.xz", self.count));
            fs::rename(&self.old_path, &new_path).context("failed to rename file")?;
            self.count += 1;
            swap(&mut new_path, &mut self.active_path);
            self.old_path = new_path;
        }
        swap(&mut self.active_file, &mut self.old_file);
        self.old_file
            .rewind()
            .context("failed to seek old file to begin")?;
        self.old_file
            .set_len(0)
            .context("failed to set old file len")?;

        log::info!("saved tmp data to {}", self.active_path.display());
        Ok(())
    }
    fn run(mut self) {
        while {
            let (lg, t) = self
                .cancal
                .1
                .wait_timeout(self.cancal.0.lock().unwrap(), self.duration)
                .unwrap();
            !*lg && t.timed_out()
        } {
            if let Err(e) = self.save_data() {
                log::error!("failed to save data to tmp dir: {:?}", e);
            }
        }
        if let Err(e) = fs::remove_dir_all(self.tmp_dest) {
            log::error!("failed to remove tmp dir: {:?}", e);
        }
    }
}

#[pyclass]
pub struct Recorder {
    data: Data,
    dirty: bool,
    tar_dest: PathBuf,
    cancel: Cancal,
    handle: Option<std::thread::JoinHandle<()>>,
}
impl Recorder {
    fn new_impl(dest: &str, last_log: Option<&str>, duration: u32) -> anyhow::Result<Self> {
        let data = Arc::new(RwLock::new(match last_log {
            Some(p) => ciborium::de::from_reader(xz2::read::XzDecoder::new(io::BufReader::new(
                fs::File::open(p).context("failed to open log file")?,
            )))
            .context("failed to deserialize log file")?,
            None => http_recorder::HttpRecord::default(),
        }));
        let cancel = Arc::new((Mutex::new(false), Condvar::new()));
        Ok(Self {
            data: Arc::clone(&data),
            dirty: false,
            tar_dest: PathBuf::from(dest),
            cancel: Arc::clone(&cancel),
            handle: Some(
                std::thread::Builder::new()
                    .name(String::from("background-saver"))
                    .spawn({
                        let saver = Saver::new(
                            cancel,
                            data,
                            std::time::Duration::from_secs(duration as u64),
                        )?;
                        move || Saver::run(saver)
                    })
                    .context("failed to spawn thrad")?,
            ),
        })
    }
    fn add_flow_impl(&mut self, flow: Flow<'_>) -> anyhow::Result<()> {
        let mut lg = self.data.write().unwrap();
        lg.entries.push(flow.to_entry()?);
        self.dirty = true;
        Ok(())
    }
    fn save_tar_impl(&mut self) -> anyhow::Result<()> {
        let lg = self.data.read().unwrap();
        lg.write_tar(xz2::write::XzEncoder::new(
            io::BufWriter::new(fs::File::create(&self.tar_dest).context("failed to create file")?),
            9,
        ))
        .context("failed to write tar")?
        .finish()
        .context("failed to finish compression")?
        .into_inner()
        .context("failed to flush buffer")?;
        self.dirty = false;
        Ok(())
    }
    fn stop_auto_save(&mut self) {
        *self.cancel.0.lock().unwrap() = true;
        self.cancel.1.notify_all();
        if let Some(h) = self.handle.take() {
            h.join().unwrap();
        }
    }
    fn finish_impl(&mut self) {
        if self.dirty {
            let _ = self.save_tar_impl();
        }
        self.stop_auto_save();
    }
}

#[pymethods]
impl Recorder {
    #[new]
    pub fn new(dest: &str, duration: u32, last_log: Option<&str>) -> anyhow::Result<Self> {
        Self::new_impl(dest, last_log, duration)
    }
    pub fn add_flow(&mut self, flow: Flow<'_>) -> anyhow::Result<()> {
        self.add_flow_impl(flow)
    }
    pub fn save_tar(&mut self) -> anyhow::Result<()> {
        self.save_tar_impl()
    }
    pub fn finish(&mut self) {
        self.finish_impl()
    }
}

#[pymodule]
#[pyo3(name = "http_recorder")]
pub fn module(_: pyo3::Python, m: &pyo3::types::PyModule) -> pyo3::PyResult<()> {
    pyo3_log::init();
    m.add_class::<Recorder>()
}
