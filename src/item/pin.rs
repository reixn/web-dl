use crate::{
    element::{comment, Author, Comment, Content},
    id,
    meta::Version,
    raw_data::{FromRaw, RawData, StrU64},
    store::storable,
};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt::Display};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PinId(pub u64);
impl Display for PinId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub const CONTENT_VERSION: Version = Version { major: 1, minor: 0 };
#[derive(Debug)]
pub struct PinContent {
    pub version: Version,
    pub content_html: Content,
}
impl id::HasId for PinContent {
    const TYPE: &'static str = "pin_content";
    type Id<'a> = id::Fixed<"pin_content">;
    fn id(&self) -> Self::Id<'_> {
        id::Fixed
    }
}
const CONTENT_HTML_DIR: &str = "content_html";
impl storable::Storable for PinContent {
    fn load<P: AsRef<std::path::Path>>(
        path: P,
        load_opt: storable::LoadOpt,
    ) -> storable::Result<Self> {
        use storable::*;
        let mut path = path.as_ref().to_path_buf();
        Ok(Self {
            version: Version::load(&path)?,
            content_html: {
                path.push(CONTENT_HTML_DIR);
                load_object(&path, load_opt, "content_html")?
            },
        })
    }
    fn store<P: AsRef<std::path::Path>>(&self, path: P) -> storable::Result<()> {
        use storable::*;
        let mut path = path.as_ref().to_path_buf();
        self.version.store(&path)?;
        path.push(CONTENT_HTML_DIR);
        store_object_to(&self.content_html, path, "content_html")
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PinInfo {
    pub id: PinId,
    pub repin_id: Option<PinId>,
    pub author: Author,
    pub created_time: DateTime<FixedOffset>,
    pub updated_time: DateTime<FixedOffset>,
}
const PIN_INFO_FILE: &str = "pin_info.yaml";

#[derive(Debug)]
pub struct PinBody {
    pub info: PinInfo,
    pub content: PinContent,
}
impl id::HasId for PinBody {
    const TYPE: &'static str = "body";
    type Id<'a> = id::Fixed<"body">;
    fn id(&self) -> Self::Id<'_> {
        id::Fixed
    }
}
impl storable::Storable for PinBody {
    fn load<P: AsRef<std::path::Path>>(
        path: P,
        load_opt: storable::LoadOpt,
    ) -> storable::Result<Self> {
        use storable::*;
        let path = path.as_ref().to_path_buf();
        Ok(Self {
            info: load_yaml(&path, PIN_INFO_FILE)?,
            content: load_fixed_id_obj(path, load_opt, "content")?,
        })
    }
    fn store<P: AsRef<std::path::Path>>(&self, path: P) -> storable::Result<()> {
        use storable::*;
        let path = path.as_ref().to_path_buf();
        store_yaml(&self.info, &path, PIN_INFO_FILE)?;
        store_object(&self.content, path, "content")
    }
}

pub const VERSION: Version = Version { major: 1, minor: 0 };

#[derive(Debug)]
pub struct Pin {
    pub version: Version,
    pub body: PinBody,
    pub repin: Option<PinBody>,
    pub comments: Vec<Comment>,
    pub raw_data: Option<RawData>,
}
impl id::HasId for Pin {
    const TYPE: &'static str = "pin";
    type Id<'a> = PinId;
    fn id(&self) -> Self::Id<'_> {
        self.body.info.id
    }
}

const REPIN_DIR: &str = "repin";
impl storable::Storable for Pin {
    fn load<P: AsRef<std::path::Path>>(
        path: P,
        load_opt: storable::LoadOpt,
    ) -> storable::Result<Self> {
        use storable::*;
        let path = path.as_ref().to_path_buf();
        let body: PinBody = PinBody::load(&path, load_opt)?;
        Ok(Self {
            version: Version::load(&path)?,
            raw_data: RawData::load_if(&path, load_opt)?,
            repin: if body.info.repin_id.is_some() {
                Some(load_object(push_path(&path, REPIN_DIR), load_opt, "repin")?)
            } else {
                None
            },
            comments: load_fixed_id_obj(path, load_opt, "comments")?,
            body,
        })
    }
    fn store<P: AsRef<std::path::Path>>(&self, path: P) -> storable::Result<()> {
        use storable::*;
        let path = path.as_ref().to_path_buf();
        self.version.store(&path)?;
        self.body.store(&path)?;
        match &self.repin {
            Some(p) => store_object_to(p, push_path(&path, REPIN_DIR), "repin")?,
            None => (),
        }
        RawData::store_option(&self.raw_data, &path)?;
        store_object(&self.comments, path, "comments")
    }
}
impl super::Fetchable for Pin {
    async fn fetch<'a>(
        client: &crate::request::Client,
        id: PinId,
    ) -> Result<serde_json::Value, reqwest::Error> {
        client
            .http_client
            .get(format!("https://www.zhihu.com/api/v4/v2/pins/{}", id))
            .send()
            .await?
            .json()
            .await
    }
}

#[derive(Deserialize)]
pub struct Reply {
    id: StrU64,
    author: FromRaw<Author>,
    created: FromRaw<DateTime<FixedOffset>>,
    updated: FromRaw<DateTime<FixedOffset>>,
    content_html: FromRaw<Content>,
    #[serde(default)]
    repin: Option<Box<Reply>>,
}
impl super::Item for Pin {
    type Reply = Reply;
    fn from_reply(mut reply: Self::Reply, raw_data: RawData) -> Self {
        fn to_body(data: Reply, repin_id: Option<PinId>) -> PinBody {
            PinBody {
                info: PinInfo {
                    id: PinId(data.id.0),
                    repin_id,
                    author: data.author.0,
                    created_time: data.created.0,
                    updated_time: data.updated.0,
                },
                content: PinContent {
                    version: CONTENT_VERSION,
                    content_html: data.content_html.0,
                },
            }
        }
        let repin = reply.repin.map(|v| to_body(*v, None));
        reply.repin = None;
        Pin {
            version: VERSION,
            body: to_body(reply, repin.as_ref().map(|v| v.info.id)),
            repin,
            comments: Vec::new(),
            raw_data: Some(raw_data),
        }
    }
    async fn get_comments<P: crate::progress::ItemProg>(
        &mut self,
        client: &crate::request::Client,
        prog: &P,
    ) -> Result<(), crate::element::comment::FetchError> {
        self.comments = Comment::get(
            client,
            prog.start_comment_tree(),
            comment::RootType::Pin,
            self.body.info.id,
        )
        .await?;
        Ok(())
    }
    async fn get_images<P: crate::progress::ItemProg>(
        &mut self,
        client: &crate::request::Client,
        prog: &P,
    ) -> bool {
        let self_url = self.body.content.content_html.image_urls();
        let repin_url = self
            .repin
            .as_ref()
            .map_or(HashSet::new(), |v| v.content.content_html.image_urls());
        let mut p = prog.start_images((self_url.len() + repin_url.len()) as u64);
        self.body
            .content
            .content_html
            .fetch_images(client, &mut p, self_url)
            .await
            | match &mut self.repin {
                Some(b) => {
                    b.content
                        .content_html
                        .fetch_images(client, &mut p, repin_url)
                        .await
                }
                None => false,
            }
    }
}
