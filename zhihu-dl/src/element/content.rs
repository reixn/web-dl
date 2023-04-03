use crate::{meta::Version, progress, raw_data::FromRaw, request::Client};
use html5ever::{
    local_name,
    tendril::Tendril,
    tokenizer::{
        BufferQueue, Tag, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
    },
};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use web_dl_base::{
    media::{fetch_images_iter, HasImage, ImageRef},
    storable::Storable,
};

pub mod document;
mod html_reader;
pub mod writer {
    pub mod pandoc;
}

pub const VERSION: Version = Version { major: 1, minor: 1 };

#[derive(Debug, Clone, Storable, HasImage, Serialize, Deserialize)]
#[store(format = "yaml")]
pub struct ContentInfo {
    pub is_empty: bool,
    #[has_image]
    pub images: Vec<ImageRef>,
}

#[derive(Debug, Clone, Storable, HasImage, Serialize, Deserialize)]
pub struct Content {
    #[store(path(ext = "yaml"))]
    pub version: Version,
    #[store(path(ext = "yaml"))]
    #[has_image(error = "pass_through")]
    pub info: ContentInfo,
    #[store(path(ext = "ron"))]
    pub document: Option<document::Document>,
    #[store(path(ext = "html"))]
    pub raw_html: Option<String>,
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
                document: None,
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
            document: None,
            raw_html: None,
        }
    }
}

impl Content {
    pub fn convert_html(&mut self) {
        self.document = self.raw_html.as_ref().map(|h| {
            let mp = self
                .info
                .images
                .iter()
                .map(|i| (i.url.as_str(), i))
                .collect();
            html_reader::from_raw_html(h.as_str(), &mp)
        });
    }
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
                        if let Some(u) = url {
                            self.0.insert(u);
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
    ) -> bool {
        if urls.is_empty() {
            false
        } else {
            self.info.images =
                fetch_images_iter(&client.http_client, images_prog, urls.into_iter()).await;
            true
        }
    }
}

pub trait HasContent {
    fn convert_html(&mut self);
}
impl<I: HasContent> HasContent for Vec<I> {
    fn convert_html(&mut self) {
        for i in self {
            i.convert_html()
        }
    }
}
impl<I: HasContent> HasContent for Option<I> {
    fn convert_html(&mut self) {
        if let Some(v) = self {
            v.convert_html()
        }
    }
}
