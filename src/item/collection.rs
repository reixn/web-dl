use crate::{
    element::{comment, Author, Comment, Content},
    id,
    meta::Version,
    raw_data::{FromRaw, RawData},
    store::storable,
};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CollectionId(pub u64);
impl Display for CollectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionInfo {
    pub id: CollectionId,
    pub title: String,
    pub creator: Author,
    pub created_time: DateTime<FixedOffset>,
    pub updated_time: DateTime<FixedOffset>,
}

pub const VERSION: Version = Version { major: 1, minor: 0 };
#[derive(Debug)]
pub struct Collection {
    pub version: Version,
    pub info: CollectionInfo,
    pub description: Content,
    pub comments: Vec<Comment>,
    pub raw_data: Option<RawData>,
}

impl id::HasId for Collection {
    const TYPE: &'static str = "collection";
    type Id<'a> = CollectionId;
    fn id(&self) -> CollectionId {
        self.info.id
    }
}
const COLLECTION_INFO_FILE: &str = "collection_info.yaml";
impl storable::Storable for Collection {
    fn load<P: AsRef<std::path::Path>>(
        path: P,
        load_opt: storable::LoadOpt,
    ) -> storable::Result<Self> {
        use storable::*;
        let path = path.as_ref().to_path_buf();
        Ok(Self {
            version: Version::load(&path)?,
            info: load_yaml(&path, COLLECTION_INFO_FILE)?,
            description: load_fixed_id_obj(path.clone(), load_opt, "description")?,
            raw_data: RawData::load_if(&path, load_opt)?,
            comments: load_fixed_id_obj(path, load_opt, "comments")?,
        })
    }
    fn store<P: AsRef<std::path::Path>>(&self, path: P) -> storable::Result<()> {
        use storable::*;
        let path = path.as_ref().to_path_buf();
        self.version.store(&path)?;
        store_yaml(&self.info, &path, COLLECTION_INFO_FILE)?;
        RawData::store_option(&self.raw_data, &path)?;
        store_object(&self.description, path.clone(), "description")?;
        store_object(&self.comments, path, "comments")
    }
}
has_image!(Collection {
    description: image(),
    comments: image()
});

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
