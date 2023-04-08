use crate::{
    element::{comment, content::HasContent, Author, Comment, Content},
    meta::Version,
    raw_data::{self, FromRaw, RawData},
    store::{self, BasicStoreItem, StoreItemContainer},
};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr};
use web_dl_base::{
    id::{HasId, OwnedId},
    media::HasImage,
    storable::Storable,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CollectionId(pub u64);
impl Display for CollectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for CollectionId {
    type Err = <u64 as FromStr>::Err;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        u64::from_str(s).map(Self)
    }
}
impl OwnedId<Collection> for CollectionId {
    fn to_id(&self) -> <Collection as HasId>::Id<'_> {
        *self
    }
}

#[derive(Debug, Storable, Serialize, Deserialize)]
#[store(format = "yaml")]
pub struct CollectionInfo {
    pub id: CollectionId,
    pub title: String,
    pub creator: Author,
    pub created_time: DateTime<FixedOffset>,
    pub updated_time: DateTime<FixedOffset>,
}

pub const VERSION: Version = Version { major: 1, minor: 0 };
#[derive(Debug, Storable, HasImage, Serialize, Deserialize)]
pub struct Collection {
    #[store(path(ext = "yaml"))]
    pub version: Version,
    #[store(path(ext = "yaml"))]
    pub info: CollectionInfo,
    #[has_image]
    pub description: Content,
    #[has_image]
    pub comments: Vec<Comment>,
    #[store(raw_data)]
    pub raw_data: Option<RawData>,
}

impl HasId for Collection {
    const TYPE: &'static str = "collection";
    type Id<'a> = CollectionId;
    fn id(&self) -> CollectionId {
        self.info.id
    }
}

impl BasicStoreItem for Collection {
    fn in_store(id: Self::Id<'_>, info: &crate::store::ObjectInfo) -> bool {
        info.collection.contains(&id)
    }
    fn add_info(&self, info: &mut crate::store::ObjectInfo) {
        info.collection.insert(self.info.id);
    }
}

impl super::Fetchable for Collection {
    async fn fetch<'a>(
        client: &crate::request::Client,
        id: CollectionId,
    ) -> Result<serde_json::Value, reqwest::Error> {
        client
            .http_client
            .get(format!("https://www.zhihu.com/api/v4/collections/{}", id))
            .send()
            .await?
            .json()
            .await
    }
}
impl HasContent for Collection {
    fn convert_html(&mut self) {
        self.description.convert_inline();
        self.comments.convert_html();
    }
    fn get_main_content(&self) -> Option<&'_ Content> {
        Some(&self.description)
    }
}

#[derive(Deserialize)]
struct Reply {
    id: u64,
    title: String,
    creator: FromRaw<Author>,
    description: FromRaw<Content>,
    created_time: FromRaw<DateTime<FixedOffset>>,
    updated_time: FromRaw<DateTime<FixedOffset>>,
}
#[derive(Deserialize)]
pub struct Wrapper {
    collection: Reply,
}
impl super::Item for Collection {
    type Reply = Wrapper;
    fn from_reply(reply: Self::Reply, raw_data: RawData) -> Self {
        let d = reply.collection;
        Collection {
            version: VERSION,
            info: CollectionInfo {
                id: CollectionId(d.id),
                title: d.title,
                creator: d.creator.0,
                created_time: d.created_time.0,
                updated_time: d.updated_time.0,
            },
            description: d.description.0,
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
            comment::RootType::Collection,
            self.info.id,
        )
        .await?;
        Ok(())
    }
    async fn get_images<P: crate::progress::ItemProg>(
        &mut self,
        client: &crate::request::Client,
        prog: &P,
    ) -> bool {
        let u = self.description.image_urls();
        self.description
            .fetch_images(client, &mut prog.start_images(u.len() as u64), u)
            .await
    }
}

impl StoreItemContainer<super::VoidOpt, super::any::Any> for Collection {
    const OPTION_NAME: &'static str = "item";
    fn in_store(id: Self::Id<'_>, info: &store::ContainerInfo) -> bool {
        info.collection.contains(&id)
    }
    fn add_info(id: Self::Id<'_>, info: &mut store::ContainerInfo) {
        info.collection.insert(id);
    }
}
impl super::ItemContainer<super::VoidOpt, super::any::Any> for Collection {
    async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
        client: &crate::request::Client,
        prog: &P,
        id: Self::Id<'a>,
    ) -> Result<std::collections::LinkedList<RawData>, reqwest::Error> {
        client
            .get_paged::<{ raw_data::Container::Collection }, _, _>(
                prog.start_fetch(),
                format!("https://www.zhihu.com/api/v4/collections/{}/items", id),
            )
            .await
    }
    fn parse_item(raw_data: RawData) -> Result<super::any::Any, serde_json::Error> {
        use super::{
            any::{self, Any},
            Item,
        };
        #[derive(Deserialize)]
        struct Reply {
            content: any::Reply,
        }
        Reply::deserialize(&raw_data.data).map(|r| Any::from_reply(r.content, raw_data))
    }
}
