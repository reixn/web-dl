use crate::{
    element::{comment, Author, Comment, Content},
    meta::Version,
    raw_data::{FromRaw, RawData},
    request::Zse96V3,
};
use chrono::{DateTime, FixedOffset};
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use web_dl_base::{id::HasId, media::HasImage, storable::Storable};

const VERSION: Version = Version { major: 1, minor: 0 };

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AnswerId(pub u64);
impl Display for AnswerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnsweredQuestion {
    id: crate::item::question::QuestionId,
    title: String,
}

#[derive(Debug, Storable, Serialize, Deserialize)]
#[store(format = "yaml")]
pub struct AnswerInfo {
    pub id: AnswerId,
    pub author: Option<Author>,
    pub question: AnsweredQuestion,
    pub created_time: DateTime<FixedOffset>,
    pub updated_time: DateTime<FixedOffset>,
}

#[derive(Debug, Storable, HasImage)]
pub struct Answer {
    #[store(path(ext = "yaml"))]
    pub version: Version,
    #[store(path(ext = "yaml"))]
    pub info: AnswerInfo,
    #[store(has_image)]
    pub content: Content,
    #[store(has_image)]
    pub comments: Vec<Comment>,
    #[store(raw_data)]
    pub raw_data: Option<RawData>,
}

impl HasId for Answer {
    const TYPE: &'static str = "answer";
    type Id<'a> = AnswerId;
    fn id(&self) -> AnswerId {
        self.info.id
    }
}

#[derive(Deserialize)]
struct ReplyQuestion {
    id: u64,
    title: String,
}
#[derive(Deserialize)]
pub struct Reply {
    id: u64,
    author: FromRaw<Option<Author>>,
    question: ReplyQuestion,
    created_time: FromRaw<DateTime<FixedOffset>>,
    updated_time: FromRaw<DateTime<FixedOffset>>,
    content: FromRaw<Content>,
}
impl super::Fetchable for Answer {
    async fn fetch<'a>(
        client: &crate::request::Client,
        id: AnswerId,
    ) -> Result<serde_json::Value, reqwest::Error> {
        client
            .request_signed::<Zse96V3, _>(
                Method::GET,
                Url::parse_with_params(
                    format!("https://www.zhihu.com/api/v4/answers/{}", id).as_str(),
                    &[("include", "content;comment_count;voteup_count")],
                )
                .unwrap(),
            )
            .send()
            .await?
            .json()
            .await
    }
}
impl super::Item for Answer {
    type Reply = Reply;
    fn from_reply(reply: Self::Reply, raw_data: RawData) -> Self {
        Answer {
            version: VERSION,
            info: AnswerInfo {
                id: AnswerId(reply.id),
                author: reply.author.0,
                question: AnsweredQuestion {
                    id: crate::item::question::QuestionId(reply.question.id),
                    title: reply.question.title,
                },
                created_time: reply.created_time.0,
                updated_time: reply.updated_time.0,
            },
            content: reply.content.0,
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
            comment::RootType::Answer,
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
        let urls = self.content.image_urls();
        self.content
            .fetch_images(client, &mut prog.start_images(urls.len() as u64), urls)
            .await
    }
}
