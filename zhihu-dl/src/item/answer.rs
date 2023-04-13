use crate::{
    element::{content::HasContent, Author, Content},
    item::comment,
    meta::Version,
    raw_data::{FromRaw, RawData},
    request::Zse96V3,
    store::BasicStoreItem,
};
use chrono::{DateTime, FixedOffset};
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use std::{cell::Cell, fmt::Display, str::FromStr};
use web_dl_base::{
    id::{HasId, OwnedId},
    media::HasImage,
    storable::Storable,
};

const VERSION: Version = Version { major: 1, minor: 1 };

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AnswerId(pub u64);
impl Display for AnswerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for AnswerId {
    type Err = <u64 as FromStr>::Err;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        u64::from_str(s).map(Self)
    }
}
impl OwnedId<Answer> for AnswerId {
    fn to_id(&self) -> <Answer as HasId>::Id<'_> {
        *self
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
    #[serde(default = "comment::has_comment_default")]
    pub has_comment: Cell<bool>,
    pub created_time: DateTime<FixedOffset>,
    pub updated_time: DateTime<FixedOffset>,
}

#[derive(Debug, Storable, HasContent, HasImage, Serialize, Deserialize)]
pub struct Answer {
    #[store(path(ext = "yaml"))]
    pub version: Version,
    #[store(path(ext = "yaml"))]
    pub info: AnswerInfo,
    #[has_image]
    #[content(main)]
    pub content: Content,
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
basic_store_item!(Answer, answer);

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
    comment_count: u64,
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
            .error_for_status()?
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
                has_comment: Cell::new(reply.comment_count > 0),
                created_time: reply.created_time.0,
                updated_time: reply.updated_time.0,
            },
            content: reply.content.0,
            raw_data: Some(raw_data),
        }
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
comment_store_container!(Answer, answer);
comment_container!(Answer, info.has_comment);

item_list_btree!(Answer, AnswerId);
