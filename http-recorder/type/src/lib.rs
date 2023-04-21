#![feature(type_changing_struct_update)]

use base16::encode_lower;
use serde::{Deserialize, Serialize};
use std::{
    error,
    fmt::Display,
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
    str::FromStr,
};
use web_dl_util::bytes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpVersion {
    Http09,
    Http10,
    Http11,
    H2,
    H3,
}

#[derive(Debug)]
pub struct HttpVersionParseErr(String);
impl Display for HttpVersionParseErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid http version: {}", self.0)
    }
}
impl error::Error for HttpVersionParseErr {}

impl FromStr for HttpVersion {
    type Err = HttpVersionParseErr;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "HTTP/0.9" => Ok(Self::Http09),
            "HTTP/1.0" => Ok(Self::Http10),
            "HTTP/1.1" => Ok(Self::Http11),
            "HTTP/2.0" => Ok(Self::H2),
            "HTTP/3.0" => Ok(Self::H3),
            v => Err(HttpVersionParseErr(v.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Options,
    Connect,
    Patch,
    Trace,
    Extension(Box<str>),
}
impl FromStr for Method {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "GET" => Self::Get,
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "DELETE" => Self::Delete,
            "HEAD" => Self::Head,
            "OPTIONS" => Self::Options,
            "CONNECT" => Self::Connect,
            "PATCH" => Self::Patch,
            "TRACE" => Self::Trace,
            v => Self::Extension(v.to_owned().into()),
        })
    }
}
pub mod header {
    use serde::{Deserialize, Serialize};

    mod name;
    pub use name::*;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub enum HeaderValue {
        Text(String),
        Binary(Box<[u8]>),
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct Header {
        pub name: HeaderName,
        pub value: HeaderValue,
    }
}
pub use header::Header;
pub type Headers = Vec<Header>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusCode(pub u16);
impl PartialEq<u16> for StatusCode {
    fn eq(&self, other: &u16) -> bool {
        self.0 == *other
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryString {
    pub name: String,
    pub value: String,
}

mod serde_mime {
    use mime::Mime;
    use serde::{de, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &Mime, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(value.to_string().as_str())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Mime, D::Error> {
        struct MimeVisitor;
        impl<'de> de::Visitor<'de> for MimeVisitor {
            type Value = Mime;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("mime")
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                v.parse().map_err(E::custom)
            }
        }
        deserializer.deserialize_str(MimeVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Bytes<const N: usize>(#[serde(with = "bytes")] pub [u8; N]);
pub const SHA256_OUTPUT_SIZE: usize = 32;
pub type SHA256Digest = Bytes<SHA256_OUTPUT_SIZE>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "algo", content = "hash")]
pub enum Digest {
    SHA256(SHA256Digest),
}

#[derive(Debug, Clone)]
struct Data<'a, const D: usize> {
    digest: &'a [u8; D],
    extension: Option<&'a str>,
    data: &'a [u8],
}
impl<'a, 'b, const D: usize> PartialEq<Data<'b, D>> for Data<'a, D> {
    fn eq(&self, other: &Data<'b, D>) -> bool {
        self.digest == other.digest && self.extension == other.extension
    }
}
impl<'a, const D: usize> Eq for Data<'a, D> {}
impl<'a, 'b, const D: usize> PartialOrd<Data<'b, D>> for Data<'a, D> {
    fn partial_cmp(&self, other: &Data<'b, D>) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering;
        Some(match self.digest.cmp(&other.digest) {
            Ordering::Equal => self.extension.cmp(&other.extension),
            v => v,
        })
    }
}
impl<'a, 'b, const D: usize> Ord for Data<'a, D> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match self.digest.cmp(&other.digest) {
            Ordering::Equal => self.extension.cmp(&other.extension),
            v => v,
        }
    }
}

fn append_file<W: Write, P: AsRef<Path>>(
    builder: &mut tar::Builder<W>,
    path: P,
    data: &[u8],
) -> std::io::Result<()> {
    let mut header = tar::Header::new_old();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    builder.append_data(&mut header, path, data)
}
impl<'a, const D: usize> Data<'a, D> {
    fn write_tar<W: Write>(
        &self,
        builder: &mut tar::Builder<W>,
        path: &mut PathBuf,
    ) -> std::io::Result<()> {
        path.push(encode_lower(&self.digest));
        if let Some(ext) = self.extension {
            path.set_extension(ext);
        }
        append_file(builder, path.as_path(), self.data)?;
        path.pop();
        Ok(())
    }
}
#[derive(Debug, Clone, Default)]
struct DataMap<'a> {
    sha256: Vec<Data<'a, SHA256_OUTPUT_SIZE>>,
}
impl<'a> DataMap<'a> {
    fn write_tar<W: Write>(&mut self, builder: &mut tar::Builder<W>) -> std::io::Result<()> {
        fn append_dir<W: Write, P: AsRef<Path>>(
            builder: &mut tar::Builder<W>,
            path: P,
        ) -> std::io::Result<()> {
            let mut h = tar::Header::new_gnu();
            h.set_entry_type(tar::EntryType::Directory);
            h.set_mode(0o755);
            builder.append_data(&mut h, path, std::io::empty())
        }

        let mut p = PathBuf::with_capacity(4 + 1 + 6 + 2 + 1 + 2 + 1 + 64 + 10);
        p.push("data");
        append_dir(builder, p.as_path())?;

        self.sha256.sort();
        self.sha256.dedup();
        if let Some(d) = self.sha256.first() {
            use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
            let mut first: u8;
            let mut second: u8;
            p.push("sha256");
            append_dir(builder, p.as_path())?;

            first = d.digest[0];
            p.push(OsStr::from_bytes(&base16::encode_byte_l(first)));
            append_dir(builder, p.as_path())?;

            second = d.digest[1];
            p.push(OsStr::from_bytes(&base16::encode_byte_l(second)));
            append_dir(builder, p.as_path())?;

            d.write_tar(builder, &mut p)?;

            for d in self.sha256.iter().skip(1) {
                if first != d.digest[0] {
                    p.pop();
                    first = d.digest[0];
                    second = d.digest[1];
                    p.set_file_name(OsStr::from_bytes(&base16::encode_byte_l(first)));
                    append_dir(builder, p.as_path())?;
                    p.push(OsStr::from_bytes(&base16::encode_byte_l(second)));
                    append_dir(builder, p.as_path())?;
                } else if second != d.digest[1] {
                    second = d.digest[1];
                    p.set_file_name(OsStr::from_bytes(&base16::encode_byte_l(second)));
                    append_dir(builder, p.as_path())?;
                }
                d.write_tar(builder, &mut p)?;
            }
        }
        Ok(())
    }
}
mod serde_data {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub type Value = Option<Box<[u8]>>;

    pub fn serialize<S: Serializer>(value: &Value, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            Value::None.serialize(serializer)
        } else {
            value.serialize(serializer)
        }
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Value, D::Error> {
        if deserializer.is_human_readable() {
            Ok(None)
        } else {
            Value::deserialize(deserializer)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    #[serde(with = "serde_mime")]
    pub content_type: mime::Mime,
    pub digest: Digest,
    pub extension: Option<String>,
    #[serde(with = "serde_data")]
    pub data: Option<Box<[u8]>>,
}
impl Content {
    fn take_data<'a: 'b, 'b>(&'a self, data: &mut DataMap<'b>) {
        if let Some(d) = &self.data {
            let Digest::SHA256(v) = &self.digest;
            data.sha256.push(Data {
                digest: &v.0,
                extension: self.extension.as_ref().map(|v| v.as_str()),
                data: d.as_ref(),
            });
        }
    }
}

pub mod request {
    use super::{Content, DataMap, Headers, HttpVersion, Method, QueryString};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Cookie {
        pub name: String,
        pub value: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct UrlEncodedFormEntry {
        pub name: String,
        pub value: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MultipartFormEntry {
        pub name: Option<String>,
        pub file_name: Option<String>,
        pub headers: Headers,
        pub content: Content,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Body {
        Content(Content),
        UrlEncodedForm(Vec<UrlEncodedFormEntry>),
        MultipartForm(Vec<MultipartFormEntry>),
    }
    impl Body {
        pub(super) fn take_data<'a: 'b, 'b>(&'a self, data: &mut DataMap<'b>) {
            match self {
                Self::Content(c) => c.take_data(data),
                Self::MultipartForm(f) => {
                    f.iter().for_each(|v| v.content.take_data(data));
                }
                _ => (),
            }
        }
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Host {
        Domain(String),
        Addr(std::net::IpAddr),
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Url {
        pub url_text: String,

        pub scheme: String,
        pub host: Option<Host>,
        pub port: Option<u16>,
        pub path: String,
        pub query: Vec<QueryString>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Request {
        pub http_version: HttpVersion,
        pub method: Method,
        pub url: Url,
        pub headers: Headers,
        pub cookies: Vec<Cookie>,
        pub body: Option<Body>,
    }
}

mod serde_date_time {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        value: &DateTime<Utc>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(
                value
                    .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, false)
                    .as_str(),
            )
        } else {
            chrono::serde::ts_microseconds::serialize(value, serializer)
        }
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<DateTime<Utc>, D::Error> {
        if deserializer.is_human_readable() {
            DateTime::<Utc>::deserialize(deserializer)
        } else {
            chrono::serde::ts_microseconds::deserialize(deserializer)
        }
    }
}

pub mod response {
    use super::{serde_date_time, Content, Headers, HttpVersion, StatusCode};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum SameSite {
        Strict,
        Lax,
        None,
    }
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub enum Expiration {
        DateTime(#[serde(with = "serde_date_time")] chrono::DateTime<chrono::Utc>),
        Session,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Cookie {
        pub name: String,
        pub value: String,
        pub domain: Option<String>,
        pub path: Option<String>,
        pub http_only: Option<bool>,
        pub secure: Option<bool>,
        pub same_site: Option<SameSite>,
        pub max_age: Option<std::time::Duration>,
        pub expires: Option<Expiration>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Response {
        pub http_version: HttpVersion,
        pub status: StatusCode,
        pub headers: Headers,
        pub cookies: Vec<Cookie>,
        pub content: Option<Content>,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timings {
    #[serde(with = "serde_date_time")]
    pub start_time: chrono::DateTime<chrono::Utc>,
    #[serde(with = "serde_date_time")]
    pub finish_time: chrono::DateTime<chrono::Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub client_addr: SocketAddr,
    pub server_addr: SocketAddr,
    pub timings: Timings,
    pub request: request::Request,
    pub response: response::Response,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}
pub const VERSION: Version = Version { major: 0, minor: 1 };

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRecord {
    pub version: Version,
    pub entries: Vec<Entry>,
}
impl Default for HttpRecord {
    fn default() -> Self {
        Self {
            version: VERSION,
            entries: Vec::new(),
        }
    }
}
impl HttpRecord {
    pub fn write_tar<W: Write>(&self, writer: W) -> std::io::Result<W> {
        let mut builder = tar::Builder::new(writer);
        {
            let mut data = DataMap::default();
            self.entries.iter().for_each(|v| {
                if let Some(v) = &v.request.body {
                    v.take_data(&mut data);
                }
                if let Some(v) = &v.response.content {
                    v.take_data(&mut data);
                }
            });
            data.write_tar(&mut builder)?;
        }
        append_file(
            &mut builder,
            "entries.json",
            serde_json::to_vec(&self.entries).unwrap().as_slice(),
        )?;
        append_file(
            &mut builder,
            "version.json",
            serde_json::to_vec(&self.version).unwrap().as_slice(),
        )?;
        builder.into_inner()
    }
}
