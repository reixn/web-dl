use crate::{
    element::{comment, Author, Comment, Content},
    meta::Version,
    raw_data::{self, FromRaw, RawData},
    request::Zse96V3,
    store::BasicStoreItem,
};
use chrono::{DateTime, FixedOffset};
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use web_dl_base::{id::HasId, media::HasImage, storable::Storable};

pub const VERSION: Version = Version { major: 1, minor: 0 };

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct QuestionId(pub u64);
impl Display for QuestionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Storable, Serialize, Deserialize)]
#[store(format = "yaml")]
pub struct QuestionInfo {
    pub id: QuestionId,
    pub title: String,
    pub author: Option<Author>,
    pub created_time: DateTime<FixedOffset>,
    pub updated_time: DateTime<FixedOffset>,
}

#[derive(Debug, Storable, HasImage, Serialize, Deserialize)]
pub struct Question {
    #[store(path(ext = "yaml"))]
    pub version: Version,
    #[store(path(ext = "yaml"))]
    pub info: QuestionInfo,
    #[has_image]
    pub content: Content,
    #[has_image]
    pub comments: Vec<Comment>,
    #[store(raw_data)]
    pub raw_data: Option<RawData>,
}
impl HasId for Question {
    const TYPE: &'static str = "question";
    type Id<'a> = QuestionId;
    fn id(&self) -> Self::Id<'_> {
        self.info.id
    }
}
impl BasicStoreItem for Question {
    fn in_store(id: Self::Id<'_>, info: &crate::store::StoreObject) -> bool {
        info.question.contains(&id)
    }
    fn add_info(&self, info: &mut crate::store::StoreObject) {
        info.question.insert(self.info.id);
    }
}

impl super::Fetchable for Question {
    async fn fetch<'a>(
        client: &crate::request::Client,
        id: QuestionId,
    ) -> Result<serde_json::Value, reqwest::Error> {
        client
            .request_signed::<Zse96V3, _>(
                Method::GET,
                Url::parse_with_params(
                    format!("https://www.zhihu.com/api/v4/questions/{}", id).as_str(),
                    &[(
                        "include",
                        "author,description,is_anonymous;detail;comment_count;answer_count;excerpt",
                    )],
                )
                .unwrap(),
            )
            .send()
            .await?
            .json()
            .await
    }
}
#[derive(Deserialize)]
pub struct Reply {
    id: u64,
    title: String,
    author: FromRaw<Option<Author>>,
    created: FromRaw<DateTime<FixedOffset>>,
    #[serde(default)]
    updated_time: FromRaw<DateTime<FixedOffset>>,
    detail: FromRaw<Content>,
}
impl super::Item for Question {
    type Reply = Reply;
    fn from_reply(reply: Self::Reply, raw_data: RawData) -> Self {
        Self {
            version: VERSION,
            info: QuestionInfo {
                id: QuestionId(reply.id),
                title: reply.title,
                author: reply.author.0,
                created_time: reply.created.0,
                updated_time: reply.updated_time.0,
            },
            content: reply.detail.0,
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
            comment::RootType::Question,
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
        let u = self.content.image_urls();
        self.content
            .fetch_images(client, &mut prog.start_images(u.len() as u64), u)
            .await
    }
}

mod param;
impl super::ItemContainer<super::answer::Answer, super::VoidOpt> for Question {
    async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
        client: &crate::request::Client,
        prog: &P,
        id: Self::Id<'a>,
        _: super::VoidOpt,
    ) -> Result<std::collections::LinkedList<RawData>, reqwest::Error> {
        client
            .get_paged_sign::<{ raw_data::Container::Question }, Zse96V3, _, _>(
                prog.start_fetch(),
                Url::parse_with_params(
                    format!("https://www.zhihu.com/api/v4/questions/{}/answers", id).as_str(),
                    &[("include", param::ANSWER_INCLUDE)],
                )
                .unwrap(),
            )
            .await
    }
}
