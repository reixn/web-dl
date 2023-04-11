use crate::{
    element::{
        comment::{self, Comment, HasComment},
        content::HasContent,
        Author, Content,
    },
    meta::Version,
    progress,
    raw_data::{FromRaw, RawData},
    request::Client,
    store::BasicStoreItem,
};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr};
use web_dl_base::{
    id::{HasId, OwnedId},
    media::{HasImage, Image},
    storable::Storable,
};

pub const VERSION: Version = Version { major: 1, minor: 1 };

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ArticleId(pub u64);
impl Display for ArticleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for ArticleId {
    type Err = <u64 as FromStr>::Err;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        u64::from_str(s).map(Self)
    }
}
impl OwnedId<Article> for ArticleId {
    fn to_id(&self) -> <Article as HasId>::Id<'_> {
        *self
    }
}

#[derive(Debug, Storable, HasImage, Serialize, Deserialize)]
#[store(format = "yaml")]
pub struct ArticleInfo {
    pub id: ArticleId,
    pub title: String,
    pub author: Author,
    #[has_image]
    pub cover: Option<Image>,
    #[serde(default = "comment::has_comment_default")]
    pub has_comment: bool,
    pub created_time: DateTime<FixedOffset>,
    pub updated_time: DateTime<FixedOffset>,
}

#[derive(Debug, Storable, HasContent, HasImage, Serialize, Deserialize)]
pub struct Article {
    #[store(path(ext = "yaml"))]
    pub version: Version,
    #[has_image(error = "pass_through")]
    #[store(path(ext = "yaml"))]
    pub info: ArticleInfo,
    #[has_image]
    #[content(main)]
    pub content: Content,
    #[has_image]
    #[content]
    pub comments: Option<Vec<Comment>>,
    #[store(raw_data)]
    pub raw_data: Option<RawData>,
}

impl HasId for Article {
    const TYPE: &'static str = "article";
    type Id<'a> = ArticleId;
    fn id(&self) -> Self::Id<'_> {
        self.info.id
    }
}
basic_store_item!(Article, article);

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
                c.fetch(&client.http_client, &mut prog.start_images(1))
                    .await;
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
    comment_count: u64,
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
impl HasComment for Article {
    fn has_comment(&self) -> bool {
        self.info.has_comment
    }
    fn is_comment_fetched(&self) -> bool {
        self.comments.is_some()
    }
    async fn get_comments<P: progress::CommentTreeProg>(
        &mut self,
        prog: P,
        client: &Client,
    ) -> Result<(), comment::fetch::Error> {
        match Comment::get(client, prog, comment::RootType::Article, self.info.id).await? {
            Some(c) => self.comments = Some(c),
            None => self.info.has_comment = false,
        }
        Ok(())
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
                has_comment: reply.comment_count > 0,
                created_time: reply.created.0,
                updated_time: reply.updated.0,
            },
            content: reply.content.0,
            comments: None,
            raw_data: Some(raw_data),
        }
    }
    async fn get_images<P: progress::ItemProg>(&mut self, client: &Client, prog: &P) -> bool {
        let u = self.content.image_urls();
        let mut prog = prog.start_images(u.len() as u64 + 1);
        self.content.fetch_images(client, &mut prog, u).await
            | match &mut self.info.cover {
                Some(c) => c.fetch(&client.http_client, &mut prog).await,
                None => false,
            }
    }
}
