use crate::{
    element::{
        comment::{self, HasComment},
        content::HasContent,
        Author, Comment, Content,
    },
    meta::Version,
    raw_data::{FromRaw, RawData, StrU64},
    store::BasicStoreItem,
};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt::Display, str::FromStr};
use web_dl_base::{
    id::{HasId, OwnedId},
    media::HasImage,
    storable::Storable,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PinId(pub u64);
impl Display for PinId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for PinId {
    type Err = <u64 as FromStr>::Err;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        u64::from_str(s).map(Self)
    }
}
impl OwnedId<Pin> for PinId {
    fn to_id(&self) -> <Pin as HasId>::Id<'_> {
        *self
    }
}

pub const CONTENT_VERSION: Version = Version { major: 1, minor: 0 };
#[derive(Debug, Storable, HasImage, HasContent, Serialize, Deserialize)]
pub struct PinContent {
    #[store(path(ext = "yaml"))]
    pub version: Version,
    #[has_image]
    #[content(main)]
    pub content_html: Content,
}

#[derive(Debug, Storable, Serialize, Deserialize)]
#[store(format = "yaml")]
pub struct PinInfo {
    pub id: PinId,
    pub repin_id: Option<PinId>,
    pub author: Author,
    #[serde(default = "comment::has_comment_default")]
    pub has_comment: bool,
    pub created_time: DateTime<FixedOffset>,
    pub updated_time: DateTime<FixedOffset>,
}

#[derive(Debug, Storable, HasImage, HasContent, Serialize, Deserialize)]
pub struct PinBody {
    #[store(path(ext = "yaml"))]
    pub info: PinInfo,
    #[has_image(error = "pass_through")]
    #[content(main)]
    pub content: PinContent,
}

pub const VERSION: Version = Version { major: 1, minor: 1 };

#[derive(Debug, Storable, HasContent, HasImage, Serialize, Deserialize)]
pub struct Pin {
    #[store(path(ext = "yaml"))]
    pub version: Version,
    #[has_image]
    #[content(main)]
    #[store(path = "flatten")]
    pub body: PinBody,
    #[has_image]
    #[content]
    pub repin: Option<PinBody>,
    #[has_image]
    #[content]
    pub comments: Option<Vec<Comment>>,
    #[store(raw_data)]
    pub raw_data: Option<RawData>,
}
impl HasId for Pin {
    const TYPE: &'static str = "pin";
    type Id<'a> = PinId;
    fn id(&self) -> Self::Id<'_> {
        self.body.info.id
    }
}
impl BasicStoreItem for Pin {
    fn in_store(id: Self::Id<'_>, info: &crate::store::ObjectInfo) -> bool {
        info.pin.contains(&id)
    }
    fn add_info(&self, info: &mut crate::store::ObjectInfo) {
        info.pin.insert(self.body.info.id);
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
impl HasComment for Pin {
    fn has_comment(&self) -> bool {
        self.body.info.has_comment
    }
    fn is_comment_fetched(&self) -> bool {
        self.comments.is_some()
    }
    async fn get_comments<P: crate::progress::CommentTreeProg>(
        &mut self,
        prog: P,
        client: &crate::request::Client,
    ) -> Result<(), comment::fetch::Error> {
        self.comments =
            Some(Comment::get(client, prog, comment::RootType::Pin, self.body.info.id).await?);
        Ok(())
    }
}

#[derive(Deserialize)]
pub struct Reply {
    id: StrU64,
    author: FromRaw<Author>,
    created: FromRaw<DateTime<FixedOffset>>,
    updated: FromRaw<DateTime<FixedOffset>>,
    comment_count: u64,
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
                    has_comment: data.comment_count > 0,
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
            comments: None,
            raw_data: Some(raw_data),
        }
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
