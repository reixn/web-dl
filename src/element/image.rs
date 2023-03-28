use crate::{
    bytes,
    progress::{self, Progress},
    raw_data::FromRaw,
    request::Client,
    store::storable,
};
use mime2ext::mime2ext;
use mime_classifier::{ApacheBugFlag, LoadContext, MimeClassifier, NoSniffFlag};
use reqwest::Url;
use serde::{de, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fmt::Display, mem::MaybeUninit, path::PathBuf};

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "algo", content = "hash")]
pub enum HashDigest {
    #[serde(rename = "sha256")]
    Sha256(#[serde(with = "bytes")] [u8; 32]),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRef {
    pub name: String,
    pub url: String,
    pub hash: HashDigest,
    #[serde(skip)]
    pub data: Option<Vec<u8>>,
}
impl ImageRef {
    pub fn store_data(&self, path: &PathBuf) -> storable::Result<()> {
        match self.data {
            Some(ref d) => storable::write_file(d, path, &self.name),
            None => Ok(()),
        }
    }
    pub fn load_data(&mut self, path: &PathBuf) -> storable::Result<()> {
        self.data = Some(storable::read_file(path, &self.name)?);
        Ok(())
    }
}

pub async fn fetch_image<P: progress::ImageProg>(
    client: &Client,
    image_prog: &mut P,
    url: Url,
) -> reqwest::Result<ImageRef> {
    let url_str = url.to_string();
    log::debug!("fetching image {}", &url_str);
    let mut resp = client.http_client.get(url).send().await?;
    image_prog.set_size(resp.content_length());
    let mut ret = match resp.content_length() {
        Some(sz) => Vec::with_capacity(sz as usize),
        None => Vec::new(),
    };
    let mut dig = Sha256::new();
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
        name: format!(
            "sha256-{}.{}",
            base16::encode_lower(hsh.as_ref()),
            mime2ext(MimeClassifier::new().classify(
                LoadContext::Image,
                NoSniffFlag::On,
                ApacheBugFlag::On,
                &None,
                &ret,
            ))
            .unwrap_or("unknown")
        ),
        url: url_str,
        hash: HashDigest::Sha256(hsh),
        data: Some(ret),
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
                prog.suspend(|| log::warn!("failed to fetch image: {}", e));
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
impl<'de> Deserialize<'de> for FromRaw<Option<Image>> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ImgVisitor;
        impl<'de> de::Visitor<'de> for ImgVisitor {
            type Value = FromRaw<Option<Image>>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("image url")
            }
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(FromRaw(if v.is_empty() {
                    None
                } else {
                    Some(Image::Url(v))
                }))
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(FromRaw(if v.is_empty() {
                    None
                } else {
                    Some(Image::Url(v.to_owned()))
                }))
            }
        }
        deserializer.deserialize_string(ImgVisitor)
    }
}
impl<'de> Deserialize<'de> for FromRaw<Image> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        FromRaw::<Option<Image>>::deserialize(deserializer).map(|e| FromRaw(e.0.unwrap()))
    }
}

impl Image {
    pub async fn fetch<P: progress::ImagesProg>(&mut self, client: &Client, images_prog: &mut P) {
        match self {
            Image::Url(u) => {
                let url = match Url::parse(u.as_str()) {
                    Ok(v) => v,
                    Err(e) => {
                        images_prog.suspend(|| log::warn!("failed to parse url {}: {}", u, e));
                        images_prog.skip();
                        return;
                    }
                };
                let mut prog = images_prog.start_image(&url);
                match fetch_image(client, &mut prog, url).await {
                    Ok(r) => *self = Self::Ref(r),
                    Err(e) => prog.suspend(|| log::warn!("failed to fetch image {}", e)),
                }
            }
            Image::Ref(_) => images_prog.skip(),
        }
    }
    pub fn load_data<C: Display>(&mut self, path: &PathBuf, context: C) -> storable::Result<()> {
        match self {
            Self::Ref(r) => r.load_data(path).map_err(|e| {
                storable::Error::load_error(context, storable::ErrorSource::Chained(Box::new(e)))
            }),
            Self::Url(_) => Ok(()),
        }
    }
    pub fn store_data<C: Display>(&self, path: &PathBuf, context: C) -> storable::Result<()> {
        match self {
            Self::Ref(r) => r.store_data(path).map_err(|e| {
                storable::Error::store_error(context, storable::ErrorSource::Chained(Box::new(e)))
            }),
            Self::Url(_) => Ok(()),
        }
    }
}
