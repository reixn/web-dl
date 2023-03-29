use crate::{
    element::{
        comment::{self, Comment},
        Author, Content,
    },
    id,
    media::Image,
    meta::Version,
    progress,
    raw_data::{FromRaw, RawData},
    request::Client,
    store::storable,
};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

pub const VERSION: Version = Version { major: 1, minor: 0 };

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ArticleId(pub u64);
impl Display for ArticleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArticleInfo {
    pub id: ArticleId,
    pub title: String,
    pub author: Author,
    pub cover: Option<Image>,
    pub created_time: DateTime<FixedOffset>,
    pub updated_time: DateTime<FixedOffset>,
}

#[derive(Debug)]
pub struct Article {
    pub version: Version,
    pub info: ArticleInfo,
    pub content: Content,
    pub comments: Vec<Comment>,
    pub raw_data: Option<RawData>,
}

impl id::HasId for Article {
    const TYPE: &'static str = "article";
    type Id<'a> = ArticleId;
    fn id(&self) -> Self::Id<'_> {
        self.info.id
    }
}

const ARTICLE_INFO_FILE: &str = "article_info.yaml";
impl storable::Storable for Article {
    fn load<P: AsRef<std::path::Path>>(
        path: P,
        load_opt: storable::LoadOpt,
    ) -> Result<Self, storable::Error> {
        use storable::*;
        let path = path.as_ref().to_path_buf();
        Ok(Self {
            version: Version::load(&path)?,
            info: load_yaml(&path, ARTICLE_INFO_FILE)?,
            raw_data: RawData::load_if(&path, load_opt)?,
            content: load_fixed_id_obj(path.clone(), load_opt, "content")?,
            comments: load_fixed_id_obj(path, load_opt, "comments")?,
        })
    }
    fn store<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), storable::Error> {
        use storable::*;
        let path = path.as_ref().to_path_buf();
        self.version.store(&path)?;
        store_yaml(&self.info, &path, ARTICLE_INFO_FILE)?;
        RawData::store_option(&self.raw_data, &path)?;
        store_object(&self.content, path.clone(), "content")?;
        store_object(&self.comments, path, "comments")
    }
}
has_image!(Article {
    info: flatten {
        cover: image(optional)
    },
    content: image(),
    comments: image()
});

impl Article {
    async fn send_request(
        client: &Client,
        id: ArticleId,
    ) -> Result<reqwest::Response, reqwest::Error> {
        log::debug!("fetching article {}", id);
        client
            .http_client
            .get(format!("https://www.zhihu.com/api/v4/articles/{}", id))
            .send()
            .await
    }
    pub async fn fix_cover<P: progress::ItemProg>(
        &mut self,
        client: &Client,
        prog: &P,
    ) -> Result<(), reqwest::Error> {
        #[derive(Deserialize)]
        struct Reply {
            title_image: FromRaw<Option<Image>>,
        }
        self.info.cover = Self::send_request(client, self.info.id)
            .await?
            .json::<Reply>()
            .await?
            .title_image
            .0;
        match &mut self.info.cover {
            Some(c) => {
                c.fetch(client, &mut prog.start_images(1)).await;
            }
            None => (),
        }
        Ok(())
    }
}
#[derive(Deserialize)]
pub struct Reply {
    id: u64,
    title: String,
    author: FromRaw<Author>,
    #[serde(default)]
    title_image: FromRaw<Option<Image>>,
    created: FromRaw<DateTime<FixedOffset>>,
    updated: FromRaw<DateTime<FixedOffset>>,
    content: FromRaw<Content>,
}
impl super::Fetchable for Article {
    async fn fetch<'a>(
        client: &crate::request::Client,
        id: ArticleId,
    ) -> Result<serde_json::Value, reqwest::Error> {
        Self::send_request(client, id).await?.json().await
    }
}
impl super::Item for Article {
    type Reply = Reply;
    fn from_reply(reply: Self::Reply, raw_data: RawData) -> Self {
        Article {
            version: VERSION,
            info: ArticleInfo {
                id: ArticleId(reply.id),
                title: reply.title,
                author: reply.author.0,
                cover: reply.title_image.0,
                created_time: reply.created.0,
                updated_time: reply.updated.0,
            },
            content: reply.content.0,
            comments: Vec::new(),
            raw_data: Some(raw_data),
        }
    }
    async fn get_comments<P: progress::ItemProg>(
        &mut self,
        client: &crate::request::Client,
        prog: &P,
    ) -> Result<(), comment::FetchError> {
        self.comments = Comment::get(
            client,
            prog.start_comment_tree(),
            comment::RootType::Article,
            self.info.id,
        )
        .await?;
        Ok(())
    }
    async fn get_images<P: progress::ItemProg>(&mut self, client: &Client, prog: &P) -> bool {
        let u = self.content.image_urls();
        let mut prog = prog.start_images(u.len() as u64 + 1);
        self.content.fetch_images(client, &mut prog, u).await
            | match &mut self.info.cover {
                Some(c) => c.fetch(client, &mut prog).await,
                None => false,
            }
    }
}
