use crate::{
    element::{comment, Author, Comment, Content},
    id,
    meta::Version,
    raw_data::{FromRaw, RawData},
    request::Zse96V3,
    store::storable,
};
use chrono::{DateTime, FixedOffset};
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

pub const VERSION: Version = Version { major: 1, minor: 0 };

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct QuestionId(pub u64);
impl Display for QuestionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionInfo {
    pub id: QuestionId,
    pub title: String,
    pub author: Option<Author>,
    pub created_time: DateTime<FixedOffset>,
    pub updated_time: DateTime<FixedOffset>,
}

#[derive(Debug)]
pub struct Question {
    pub version: Version,
    pub info: QuestionInfo,
    pub content: Content,
    pub comments: Vec<Comment>,
    pub raw_data: Option<RawData>,
}
impl id::HasId for Question {
    const TYPE: &'static str = "question";
    type Id<'a> = QuestionId;
    fn id(&self) -> Self::Id<'_> {
        self.info.id
    }
}

const QUESTION_INFO_FILE: &str = "question_info.yaml";
impl storable::Storable for Question {
    fn load<P: AsRef<std::path::Path>>(
        path: P,
        load_opt: storable::LoadOpt,
    ) -> storable::Result<Self> {
        use storable::*;
        let path = path.as_ref().to_path_buf();
        Ok(Self {
            version: Version::load(&path)?,
            info: load_yaml(&path, QUESTION_INFO_FILE)?,
            content: load_fixed_id_obj(path.clone(), load_opt, "content")?,
            raw_data: RawData::load_if(&path, load_opt)?,
            comments: load_fixed_id_obj(path, load_opt, "comments")?,
        })
    }
    fn store<P: AsRef<std::path::Path>>(&self, path: P) -> storable::Result<()> {
        use storable::*;
        let path = path.as_ref().to_path_buf();
        self.version.store(&path)?;
        store_yaml(&self.info, &path, QUESTION_INFO_FILE)?;
        store_object(&self.content, path.clone(), "content")?;
        RawData::store_option(&self.raw_data, &path)?;
        store_object(&self.comments, path, "comments")
    }
}
has_image!(Question {
    content: image(),
    comments: image()
});

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
