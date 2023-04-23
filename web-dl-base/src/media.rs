use crate::{id, progress};
use mime2ext::mime2ext;
use mime_classifier::{ApacheBugFlag, LoadContext, MimeClassifier, NoSniffFlag};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    fs, io,
    mem::MaybeUninit,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug)]
pub enum FsErrorOp {
    CreateDir,
    WriteFile,
    ReadFile,
    Canonicalize,
    HeadLinkTo(PathBuf),
}
impl Display for FsErrorOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateDir => f.write_str("create directory"),
            Self::WriteFile => f.write_str("write file"),
            Self::ReadFile => f.write_str("read file"),
            Self::Canonicalize => f.write_str("canonicalize"),
            Self::HeadLinkTo(p) => write!(f, "hard link to {} from", p.display()),
        }
    }
}
#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to {op} {}", path.display())]
    Fs {
        op: FsErrorOp,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to process {field}")]
    Chained {
        field: String,
        #[source]
        source: Box<Error>,
    },
}

pub trait StoreImage {
    fn load_images<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error>;
    fn migrate<S, P>(&self, image_store: S, path: P) -> Result<(), Error>
    where
        S: AsRef<Path>,
        P: AsRef<Path>;
    fn store_extension(&self) -> Option<&str> {
        None
    }
    fn store_images<P: AsRef<Path>>(&self, path: P) -> Result<(), Error>;
    fn drop_images(&mut self);
}
pub use web_dl_derive::StoreImage;

#[doc(hidden)]
/// private module, for derive macro only
pub mod macro_export {
    use super::{Error, StoreImage};
    pub use std::{convert::AsRef, option::Option, path::Path, result::Result, string::String};
    use std::{fmt::Display, path::PathBuf};

    pub fn create_dir_missing<P: AsRef<Path>>(path: P) -> Result<(), Error> {
        let path = path.as_ref();
        if !path.exists() {
            std::fs::create_dir_all(path).map_err(|e| Error::Fs {
                op: super::FsErrorOp::CreateDir,
                path: path.to_path_buf(),
                source: e,
            })
        } else {
            Ok(())
        }
    }
    pub fn load_img_chained<I: StoreImage, P: AsRef<Path>, C: Display>(
        field: &mut I,
        path: P,
        context: C,
    ) -> Result<(), Error> {
        field.load_images(path).map_err(|e| Error::Chained {
            field: context.to_string(),
            source: Box::new(e),
        })
    }
    pub fn with_extension<I: StoreImage>(field: &I, path: &Path, name: &str) -> PathBuf {
        let mut path = path.join(name);
        if let Some(ext) = field.store_extension() {
            path.set_extension(ext);
        }
        path
    }
    pub fn migrate_img_chained<I, S, P, C>(
        field: &I,
        image_store: S,
        path: P,
        context: C,
    ) -> Result<(), Error>
    where
        I: StoreImage,
        S: AsRef<Path>,
        P: AsRef<Path>,
        C: Display,
    {
        field
            .migrate(image_store, path)
            .map_err(|e| Error::Chained {
                field: context.to_string(),
                source: Box::new(e),
            })
    }
    pub fn store_img_chained<I: StoreImage, P: AsRef<Path>, C: Display>(
        field: &I,
        path: P,
        context: C,
    ) -> Result<(), Error> {
        field.store_images(path).map_err(|e| Error::Chained {
            field: context.to_string(),
            source: Box::new(e),
        })
    }
}
use macro_export::create_dir_missing;

impl<I: StoreImage> StoreImage for Option<I> {
    fn store_extension(&self) -> Option<&str> {
        self.as_ref().and_then(|v| v.store_extension())
    }
    fn load_images<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        match self {
            Some(i) => i.load_images(path),
            None => Ok(()),
        }
    }
    fn migrate<S, P>(&self, image_store: S, path: P) -> Result<(), Error>
    where
        S: AsRef<Path>,
        P: AsRef<Path>,
    {
        match self {
            Some(i) => i.migrate(image_store, path),
            None => Ok(()),
        }
    }
    fn drop_images(&mut self) {
        if let Some(i) = self {
            i.drop_images()
        }
    }
    fn store_images<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        match self {
            Some(i) => i.store_images(path),
            None => Ok(()),
        }
    }
}
impl<I: id::HasId + StoreImage> StoreImage for Vec<I> {
    fn load_images<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        let path = path.as_ref();
        for i in self.iter_mut() {
            let id_str = i.id().to_string();
            i.load_images(path.join(id_str.as_str()))
                .map_err(|e| Error::Chained {
                    field: id_str,
                    source: Box::new(e),
                })?;
        }
        Ok(())
    }
    fn migrate<S, P>(&self, image_store: S, path: P) -> Result<(), Error>
    where
        S: AsRef<Path>,
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        create_dir_missing(path)?;
        let image_store = image_store.as_ref();
        for i in self.iter() {
            let id_str = i.id().to_string();
            i.migrate(image_store, path.join(id_str.as_str()))
                .map_err(|e| Error::Chained {
                    field: id_str,
                    source: Box::new(e),
                })?;
        }
        Ok(())
    }
    fn store_images<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        let path = path.as_ref();
        create_dir_missing(path)?;
        for i in self.iter() {
            let id_str = i.id().to_string();
            i.store_images(path.join(id_str.as_str()))
                .map_err(|e| Error::Chained {
                    field: id_str,
                    source: Box::new(e),
                })?;
        }
        Ok(())
    }
    fn drop_images(&mut self) {
        for i in self.iter_mut() {
            i.drop_images()
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Hash, Serialize, Deserialize)]
#[serde(tag = "algo", content = "hash")]
pub enum HashDigest {
    #[serde(rename = "sha256")]
    Sha256(#[serde(with = "hex::serde")] [u8; 32]),
}
impl HashDigest {
    fn store_path(&self, parent: &Path, extension: &str) -> PathBuf {
        let mut ret = parent.to_path_buf();
        ret.push(match self {
            Self::Sha256(h) => format!("sha256-{}", base16::encode_lower(h)),
        });
        ret.set_extension(extension);
        ret
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRef {
    pub url: String,
    pub hash: HashDigest,
    pub extension: String,
    #[serde(skip)]
    pub data: Option<Box<[u8]>>,
}
impl Display for ImageRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.hash {
            HashDigest::Sha256(s) => write!(f, "sha256-{}.{}", hex::encode(s), self.extension),
        }
    }
}
impl id::HasId for ImageRef {
    const TYPE: &'static str = "image";
    type Id<'a> = &'a Self;
    fn id(&self) -> Self::Id<'_> {
        self
    }
}
impl StoreImage for ImageRef {
    fn store_extension(&self) -> Option<&str> {
        Some(self.extension.as_str())
    }
    fn load_images<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        self.data = Some(
            fs::read(path.as_ref())
                .map_err(|e| Error::Fs {
                    op: FsErrorOp::ReadFile,
                    path: path.as_ref().to_path_buf(),
                    source: e,
                })?
                .into_boxed_slice(),
        );
        Ok(())
    }
    fn migrate<S, P>(&self, image_store: S, path: P) -> Result<(), Error>
    where
        S: AsRef<Path>,
        P: AsRef<Path>,
    {
        let sp = {
            let sp = self.hash.store_path(image_store.as_ref(), &self.extension);
            sp.as_path().canonicalize().map_err(|e| Error::Fs {
                op: FsErrorOp::Canonicalize,
                path: sp,
                source: e,
            })?
        };
        fs::hard_link(sp.as_path(), path.as_ref()).map_err(|e| Error::Fs {
            op: FsErrorOp::HeadLinkTo(sp),
            path: path.as_ref().to_path_buf(),
            source: e,
        })
    }
    fn store_images<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        match &self.data {
            Some(d) => fs::write(path.as_ref(), d).map_err(|e| Error::Fs {
                op: FsErrorOp::WriteFile,
                path: path.as_ref().to_path_buf(),
                source: e,
            }),
            None => Ok(()),
        }
    }
    fn drop_images(&mut self) {
        self.data = None;
    }
}

pub async fn fetch_image<P: progress::ImageProg>(
    client: &Client,
    image_prog: &mut P,
    url: Url,
) -> reqwest::Result<ImageRef> {
    use sha2::Digest;
    let url_str = url.to_string();
    log::debug!("fetching image {}", &url_str);
    let mut resp = client.get(url).send().await?;
    image_prog.set_size(resp.content_length());
    let mut ret = match resp.content_length() {
        Some(sz) => Vec::with_capacity(sz as usize),
        None => Vec::new(),
    };
    let mut dig = sha2::Sha256::new();
    while let Some(s) = resp.chunk().await? {
        image_prog.inc(s.len() as u64);
        ret.extend_from_slice(&s);
        dig.update(&s);
    }
    let hsh = {
        let mut buf: [MaybeUninit<u8>; 32] = MaybeUninit::uninit_array();
        MaybeUninit::write_slice(&mut buf, dig.finalize().as_ref());
        unsafe { MaybeUninit::array_assume_init(buf) }
    };
    log::debug!(
        "fetched image {}, sha256: {}",
        url_str,
        base16::encode_lower(&hsh)
    );
    Ok(ImageRef {
        url: url_str,
        hash: HashDigest::Sha256(hsh),
        extension: mime2ext(MimeClassifier::new().classify(
            LoadContext::Image,
            NoSniffFlag::On,
            ApacheBugFlag::On,
            &None,
            &ret,
        ))
        .unwrap_or("unknown")
        .to_owned(),
        data: Some(ret.into_boxed_slice()),
    })
}
pub async fn fetch_images_iter<I, P>(client: &Client, images_prog: &mut P, imgs: I) -> Vec<ImageRef>
where
    I: Iterator<Item = Url>,
    P: progress::ImagesProg,
{
    let mut ret = Vec::new();
    for url in imgs {
        let mut prog = images_prog.start_image(&url);
        if url.scheme() == "data" {
            continue;
        }
        match fetch_image(client, &mut prog, url).await {
            Ok(re) => {
                ret.push(re);
            }
            Err(e) => {
                log::warn!("failed to fetch image: {:?}", anyhow::Error::new(e));
            }
        }
    }
    ret.sort_by(|a: &ImageRef, b: &ImageRef| a.hash.cmp(&b.hash));
    ret
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Image {
    #[serde(rename = "url")]
    Url(String),
    #[serde(rename = "ref")]
    Ref(ImageRef),
}
impl StoreImage for Image {
    fn store_extension(&self) -> Option<&str> {
        match self {
            Self::Ref(r) => r.store_extension(),
            Self::Url(_) => None,
        }
    }
    fn load_images<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        match self {
            Self::Ref(r) => r.load_images(path),
            Self::Url(_) => Ok(()),
        }
    }
    fn migrate<S, P>(&self, image_store: S, path: P) -> Result<(), Error>
    where
        S: AsRef<Path>,
        P: AsRef<Path>,
    {
        match self {
            Self::Ref(r) => r.migrate(image_store, path),
            Self::Url(_) => Ok(()),
        }
    }
    fn drop_images(&mut self) {
        if let Self::Ref(r) = self {
            r.drop_images()
        }
    }
    fn store_images<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        match self {
            Self::Ref(r) => r.store_images(path),
            Self::Url(_) => Ok(()),
        }
    }
}

impl Image {
    pub async fn fetch<P: progress::ImagesProg>(
        &mut self,
        client: &Client,
        images_prog: &mut P,
    ) -> bool {
        match self {
            Image::Url(u) => {
                let url = match Url::parse(u.as_str()) {
                    Ok(v) => v,
                    Err(e) => {
                        log::warn!("failed to parse url {}: {}", u, e);
                        images_prog.skip();
                        return false;
                    }
                };
                let mut prog = images_prog.start_image(&url);
                match fetch_image(client, &mut prog, url).await {
                    Ok(r) => *self = Self::Ref(r),
                    Err(e) => log::warn!("failed to fetch image {:?}", anyhow::Error::new(e)),
                }
                true
            }
            Image::Ref(_) => {
                images_prog.skip();
                false
            }
        }
    }
}
