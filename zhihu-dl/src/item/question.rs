use crate::{
    element::{content::HasContent, Author, Content},
    item::comment,
    meta::Version,
    raw_data::{self, FromRaw, RawData},
    request::Zse96V3,
    store::{BasicStoreContainer, BasicStoreItem},
};
use chrono::{DateTime, FixedOffset};
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use std::{cell::Cell, collections::BTreeSet, fmt::Display, str::FromStr};
use web_dl_base::{
    id::{HasId, OwnedId},
    media::StoreImage,
    storable::Storable,
};

pub const VERSION: Version = Version { major: 1, minor: 1 };

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct QuestionId(pub u64);
impl Display for QuestionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for QuestionId {
    type Err = <u64 as FromStr>::Err;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        u64::from_str(s).map(Self)
    }
}
impl OwnedId<Question> for QuestionId {
    fn to_id(&self) -> <Question as HasId>::Id<'_> {
        *self
    }
}

#[derive(Debug, Clone, Storable, Serialize, Deserialize)]
#[store(format = "yaml")]
pub struct QuestionInfo {
    pub id: QuestionId,
    pub title: String,
    pub author: Option<Author>,
    #[serde(default = "comment::has_comment_default")]
    pub has_comment: Cell<bool>,
    pub created_time: DateTime<FixedOffset>,
    pub updated_time: DateTime<FixedOffset>,
}

#[derive(Debug, Storable, StoreImage, HasContent, Serialize, Deserialize)]
pub struct Question {
    #[store(path(ext = "yaml"))]
    pub version: Version,
    #[store(path(ext = "yaml"))]
    pub info: QuestionInfo,
    #[has_image]
    #[content(main)]
    pub content: Content,
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
basic_store_item!(Question, question);

comment_store_container!(Question, question);
comment_container!(Question, info.has_comment);

item_list_btree!(Question, QuestionId);

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
            .error_for_status()?
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
    comment_count: u64,
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
                has_comment: Cell::new(reply.comment_count > 0),
                created_time: reply.created.0,
                updated_time: reply.updated_time.0,
            },
            content: reply.detail.0,
            raw_data: Some(raw_data),
        }
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
impl BasicStoreContainer<super::VoidOpt, super::answer::Answer> for Question {
    const OPTION_NAME: &'static str = "answer";
    type ItemList = BTreeSet<super::answer::AnswerId>;
    fn in_store(id: Self::Id<'_>, store: &crate::store::Store) -> bool {
        store.objects.question.get(&id).map_or(false, |v| v.answer)
    }
    fn add_info(id: Self::Id<'_>, store: &mut crate::store::Store) {
        store.objects.question.entry(id).or_default().answer = true;
    }
}
impl super::ItemContainer<super::VoidOpt, super::answer::Answer> for Question {
    async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
        client: &crate::request::Client,
        prog: &P,
        id: Self::Id<'a>,
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
