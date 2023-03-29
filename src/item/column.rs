use crate::{
    element::{Author, Content},
    id,
    media::Image,
    meta::Version,
    raw_data::{FromRaw, RawData},
    store::storable,
};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ColumnId(pub String);
impl Display for ColumnId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub id: ColumnId,
    pub title: String,
    pub author: Author,
    pub image: Option<Image>,
    pub created_time: DateTime<FixedOffset>,
    pub updated_time: DateTime<FixedOffset>,
}

const VERSION: Version = Version { major: 1, minor: 0 };
#[derive(Debug)]
pub struct Column {
    pub version: Version,
    pub info: ColumnInfo,
    pub intro: Content,
    pub description: Content,
    pub raw_data: Option<RawData>,
}

impl id::HasId for Column {
    const TYPE: &'static str = "column";
    type Id<'a> = &'a ColumnId;
    fn id(&self) -> Self::Id<'_> {
        &self.info.id
    }
}

const COLUMN_INFO_FILE: &str = "column_info.yaml";
const INTRO_DIR: &str = "intro";
const DESCRIPTION_DIR: &str = "description";
impl storable::Storable for Column {
    fn load<P: AsRef<std::path::Path>>(
        path: P,
        load_opt: storable::LoadOpt,
    ) -> storable::Result<Self> {
        use storable::*;
        let mut path = path.as_ref().to_path_buf();
        Ok(Self {
            version: Version::load(&path)?,
            info: load_yaml(&path, COLUMN_INFO_FILE)?,
            intro: load_object(push_path(&path, INTRO_DIR), load_opt, "intro")?,
            raw_data: RawData::load_if(&path, load_opt)?,
            description: load_object(
                {
                    path.push(DESCRIPTION_DIR);
                    path
                },
                load_opt,
                "description",
            )?,
        })
    }
    fn store<P: AsRef<std::path::Path>>(&self, path: P) -> storable::Result<()> {
        use storable::*;
        let mut path = path.as_ref().to_path_buf();
        self.version.store(&path)?;
        store_yaml(&self.info, &path, COLUMN_INFO_FILE)?;
        RawData::store_option(&self.raw_data, &path)?;
        store_object_to(&self.intro, push_path(&path, INTRO_DIR), "intro")?;
        path.push(DESCRIPTION_DIR);
        store_object_to(&self.description, path, "description")
    }
}
has_image!(Column {
    info: flatten {
        image: image(optional)
    },
    intro: image(),
    description: image()
});

impl super::Fetchable for Column {
    async fn fetch<'a>(
        client: &crate::request::Client,
        id: Self::Id<'a>,
    ) -> Result<serde_json::Value, reqwest::Error> {
        client
            .http_client
            .get(format!("https://www.zhihu.com/api/v4/columns/{}", id))
            .query(&[("include", "intro,created")])
            .send()
            .await?
            .json()
            .await
    }
}

#[derive(Deserialize)]
pub struct Reply {
    id: String,
    title: String,
    author: FromRaw<Author>,
    created: FromRaw<DateTime<FixedOffset>>,
    updated: FromRaw<DateTime<FixedOffset>>,
    image_url: FromRaw<Option<Image>>,
    intro: FromRaw<Content>,
    description: FromRaw<Content>,
}
impl super::Item for Column {
    type Reply = Reply;
    fn from_reply(reply: Self::Reply, raw_data: RawData) -> Self {
        Self {
            version: VERSION,
            info: ColumnInfo {
                id: ColumnId(reply.id),
                title: reply.title,
                author: reply.author.0,
                image: reply.image_url.0,
                created_time: reply.created.0,
                updated_time: reply.updated.0,
            },
            intro: reply.intro.0,
            description: reply.description.0,
            raw_data: Some(raw_data),
        }
    }
    async fn get_comments<P: crate::progress::ItemProg>(
        &mut self,
        _: &crate::request::Client,
        _: &P,
    ) -> Result<(), crate::element::comment::FetchError> {
        Ok(())
    }
    async fn get_images<P: crate::progress::ItemProg>(
        &mut self,
        client: &crate::request::Client,
        prog: &P,
    ) -> bool {
        use crate::progress::ImagesProg;
        let url_i = self.intro.image_urls();
        let url_d = self.description.image_urls();
        let mut prog = prog.start_images((url_i.len() + url_d.len()) as u64 + 1);
        self.intro.fetch_images(client, &mut prog, url_i).await
            | self
                .description
                .fetch_images(client, &mut prog, url_d)
                .await
            | match &mut self.info.image {
                Some(i) => i.fetch(client, &mut prog).await,
                None => {
                    prog.skip();
                    false
                }
            }
    }
}
