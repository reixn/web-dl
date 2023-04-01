use crate::{id, progress};
use mime2ext::mime2ext;
use mime_classifier::{ApacheBugFlag, LoadContext, MimeClassifier, NoSniffFlag};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, mem::MaybeUninit, rc::Rc};

mod hash;
pub use hash::HashDigest;

mod store;
pub use store::{Loader, RefSet, Storer};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Store(#[from] store::Error),
    #[error("failed to process {field}")]
    Chained {
        field: String,
        #[source]
        source: Box<Error>,
    },
}

pub trait HasImage {
    fn load_images(&mut self, loader: &mut Loader) -> Result<(), Error>;
    fn store_images(&self, storer: &mut Storer) -> Result<(), Error>;
    fn image_refs<'a, 'b>(&'b self, ref_set: &'a mut RefSet<'b>)
    where
        'b: 'a;
}
pub use web_dl_derive::HasImage;

#[doc(hidden)]
/// private module, for derive macro only
pub mod macro_export {
    use super::{Error, HasImage, Loader, Storer};
    use std::fmt::Display;
    pub use std::{option::Option, result::Result, string::String};

    pub fn load_img_chained<I: HasImage, C: Display>(
        field: &mut I,
        loader: &mut Loader,
        context: C,
    ) -> Result<(), Error> {
        field.load_images(loader).map_err(|e| Error::Chained {
            field: context.to_string(),
            source: Box::new(e),
        })
    }
    pub fn store_img_chained<I: HasImage, C: Display>(
        field: &I,
        storer: &mut Storer,
        context: C,
    ) -> Result<(), Error> {
        field.store_images(storer).map_err(|e| Error::Chained {
            field: context.to_string(),
            source: Box::new(e),
        })
    }
}

impl<I: HasImage> HasImage for Option<I> {
    fn load_images(&mut self, loader: &mut Loader) -> Result<(), Error> {
        match self {
            Some(i) => i.load_images(loader),
            None => Ok(()),
        }
    }
    fn image_refs<'a, 'b>(&'b self, ref_set: &'a mut RefSet<'b>)
    where
        'b: 'a,
    {
        match self {
            Some(i) => i.image_refs(ref_set),
            None => (),
        }
    }
    fn store_images(&self, storer: &mut Storer) -> Result<(), Error> {
        match self {
            Some(i) => i.store_images(storer),
            None => Ok(()),
        }
    }
}
impl<I: id::HasId + HasImage> HasImage for Vec<I> {
    fn load_images(&mut self, loader: &mut Loader) -> Result<(), Error> {
        for i in self.iter_mut() {
            i.load_images(loader).map_err(|e| Error::Chained {
                field: i.id().to_string(),
                source: Box::new(e),
            })?;
        }
        Ok(())
    }
    fn store_images(&self, storer: &mut Storer) -> Result<(), Error> {
        for i in self.iter() {
            i.store_images(storer).map_err(|e| Error::Chained {
                field: i.id().to_string(),
                source: Box::new(e),
            })?;
        }
        Ok(())
    }
    fn image_refs<'a, 'b>(&'b self, ref_set: &'a mut store::RefSet<'b>)
    where
        'b: 'a,
    {
        for i in self {
            i.image_refs(ref_set)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRef {
    pub url: String,
    pub hash: HashDigest,
    pub extension: String,
    #[serde(skip)]
    pub data: Option<Rc<[u8]>>,
}
impl Display for ImageRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("image {:#?}", self.hash))
    }
}
impl id::HasId for ImageRef {
    const TYPE: &'static str = "image";
    type Id<'a> = &'a Self;
    fn id(&self) -> Self::Id<'_> {
        self
    }
}
impl HasImage for ImageRef {
    fn load_images(&mut self, loader: &mut store::Loader) -> Result<(), Error> {
        self.data = Some(
            loader
                .load(&self.hash, self.extension.as_str())
                .map_err(Error::Store)?,
        );
        Ok(())
    }
    fn image_refs<'a, 'b>(&'b self, ref_set: &'a mut store::RefSet<'b>)
    where
        'b: 'a,
    {
        ref_set.add_root(&self.hash, self.extension.as_str());
    }
    fn store_images(&self, storer: &mut store::Storer) -> Result<(), Error> {
        match &self.data {
            Some(d) => storer
                .store(&self.hash, self.extension.as_str(), d)
                .map_err(Error::Store),
            None => Ok(()),
        }
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
        data: Some(Rc::from(ret.into_boxed_slice())),
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
        match fetch_image(client, &mut prog, url).await {
            Ok(re) => {
                ret.push(re);
            }
            Err(e) => {
                log::warn!("failed to fetch image: {}", e);
            }
        }
    }
    ret.sort_by(|a: &ImageRef, b: &ImageRef| a.hash.cmp(&b.hash));
    ret
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Image {
    Url(String),
    Ref(ImageRef),
}
impl HasImage for Image {
    fn load_images(&mut self, loader: &mut store::Loader) -> Result<(), Error> {
        match self {
            Image::Ref(r) => r.load_images(loader),
            Image::Url(_) => Ok(()),
        }
    }
    fn store_images(&self, storer: &mut store::Storer) -> Result<(), Error> {
        match self {
            Image::Ref(r) => r.store_images(storer),
            Image::Url(_) => Ok(()),
        }
    }
    fn image_refs<'a, 'b>(&'b self, ref_set: &'a mut store::RefSet<'b>)
    where
        'b: 'a,
    {
        match self {
            Image::Ref(r) => r.image_refs(ref_set),
            Image::Url(_) => (),
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
                    Err(e) => log::warn!("failed to fetch image {}", e),
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
