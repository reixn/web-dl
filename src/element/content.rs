use crate::{
    element::image::{fetch_images_iter, ImageRef},
    id::{self, HasId},
    meta::Version,
    progress,
    raw_data::FromRaw,
    request::Client,
    store::storable,
};
use html5ever::{
    local_name,
    tendril::Tendril,
    tokenizer::{
        BufferQueue, Tag, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
    },
};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

pub const VERSION: Version = Version { major: 1, minor: 0 };

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentInfo {
    pub is_empty: bool,
    pub images: Vec<ImageRef>,
}

#[derive(Debug, Clone)]
pub struct Content {
    pub version: Version,
    pub info: ContentInfo,
    pub raw_html: Option<String>,
}
impl HasId for Content {
    const TYPE: &'static str = "content";
    type Id<'a> = id::Fixed<"content">;
    fn id(&self) -> Self::Id<'_> {
        id::Fixed
    }
}
impl<'de> Deserialize<'de> for FromRaw<Content> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer).map(|d| {
            FromRaw(Content {
                version: VERSION,
                info: ContentInfo {
                    is_empty: d.is_empty(),
                    images: Vec::new(),
                },
                raw_html: if d.is_empty() { None } else { Some(d) },
            })
        })
    }
}
impl Default for Content {
    fn default() -> Self {
        Self {
            version: VERSION,
            info: ContentInfo {
                is_empty: true,
                images: Vec::new(),
            },
            raw_html: None,
        }
    }
}

const CONTENT_INFO_FILE: &str = "content_info.yaml";
const RAW_HTML_FILE: &str = "raw_html.html";
const IMAGES_DIR: &str = "images";
impl storable::Storable for Content {
    fn load<P: AsRef<Path>>(path: P, load_opt: storable::LoadOpt) -> Result<Self, storable::Error> {
        use storable::*;
        let path = path.as_ref().to_path_buf();
        let info = {
            let mut info: ContentInfo = load_yaml(&path, CONTENT_INFO_FILE)?;
            if load_opt.load_img && !info.is_empty {
                let mut path = path.clone();
                path.push(IMAGES_DIR);
                for i in &mut info.images {
                    i.load_data(&path).map_err(|e| {
                        Error::load_error("images", ErrorSource::Chained(Box::new(e)))
                    })?;
                }
            }
            info
        };
        Ok(Content {
            version: Version::load(&path)?,
            raw_html: if info.is_empty {
                None
            } else {
                Some(read_text_file(&path, RAW_HTML_FILE)?)
            },
            info,
        })
    }
    fn store<P: AsRef<Path>>(&self, path: P) -> Result<(), storable::Error> {
        use storable::*;
        let mut path = path.as_ref().to_path_buf();
        self.version.store(&path)?;
        store_yaml(&self.info, &path, CONTENT_INFO_FILE)?;
        match &self.raw_html {
            Some(h) => write_file(h, &path, RAW_HTML_FILE)?,
            None => (),
        }

        if !self.info.is_empty {
            path.push(IMAGES_DIR);
            create_dir_missing(&path, "image dir")?;
            for i in self.info.images.iter() {
                i.store_data(&path)
                    .map_err(|e| Error::store_error("images", ErrorSource::Chained(Box::new(e))))?;
            }
        }
        Ok(())
    }
}
impl Content {
    pub(crate) fn image_urls(&self) -> HashSet<Url> {
        let html = match &self.raw_html {
            Some(h) => h,
            None => return HashSet::default(),
        };
        struct ImageSink(HashSet<Url>);
        impl TokenSink for ImageSink {
            type Handle = ();
            fn process_token(&mut self, token: Token, _: u64) -> TokenSinkResult<Self::Handle> {
                match token {
                    Token::TagToken(Tag {
                        kind: TagKind::StartTag,
                        name: local_name!("img"),
                        attrs,
                        ..
                    }) => {
                        let mut url = None;
                        for i in attrs {
                            match i.name.local.as_bytes() {
                                b"data-original" => {
                                    url = Url::parse(
                                        std::str::from_utf8(i.value.as_bytes()).unwrap(),
                                    )
                                    .ok();
                                    break;
                                }
                                b"src" => {
                                    url = Url::parse(
                                        std::str::from_utf8(i.value.as_bytes()).unwrap(),
                                    )
                                    .ok();
                                }
                                _ => {}
                            };
                        }
                        match url {
                            Some(u) => {
                                self.0.insert(u);
                            }
                            None => {}
                        }
                        TokenSinkResult::Continue
                    }
                    _ => TokenSinkResult::Continue,
                }
            }
        }
        let mut t = Tokenizer::new(ImageSink(HashSet::new()), TokenizerOpts::default());
        let mut bq = BufferQueue::new();
        bq.push_back(Tendril::from_slice(html.as_str()));
        let _ = t.feed(&mut bq);
        t.end();
        t.sink.0
    }
    pub(crate) async fn fetch_images<P: progress::ImagesProg>(
        &mut self,
        client: &Client,
        images_prog: &mut P,
        urls: HashSet<Url>,
    ) {
        self.info.images = fetch_images_iter(client, images_prog, urls.into_iter()).await
    }
}
