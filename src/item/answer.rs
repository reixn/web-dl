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

#[derive(Debug, Serialize, Deserialize)]
pub struct AnswerInfo {
    pub id: AnswerId,
    pub author: Option<Author>,
    pub question: AnsweredQuestion,
    pub created_time: DateTime<FixedOffset>,
    pub updated_time: DateTime<FixedOffset>,
}

#[derive(Debug)]
pub struct Answer {
    pub version: Version,
    pub info: AnswerInfo,
    pub content: Content,
    pub comments: Vec<Comment>,
    pub raw_data: Option<RawData>,
}

impl id::HasId for Answer {
    const TYPE: &'static str = "answer";
    type Id<'a> = AnswerId;
    fn id(&self) -> AnswerId {
        self.info.id
    }
}

const ANSWER_INFO_FILE: &str = "answer_info.yaml";
impl storable::Storable for Answer {
    fn load<P: AsRef<std::path::Path>>(
        path: P,
        load_opt: storable::LoadOpt,
    ) -> storable::Result<Self> {
        use storable::*;
        let path = path.as_ref().to_path_buf();
        Ok(Self {
            version: Version::load(&path)?,
            raw_data: RawData::load_if(&path, load_opt)?,
            info: load_yaml(&path, ANSWER_INFO_FILE)?,
            content: load_fixed_id_obj(path.clone(), load_opt, "comment")?,
            comments: load_fixed_id_obj(path, load_opt, "content")?,
        })
    }
    fn store<P: AsRef<std::path::Path>>(&self, path: P) -> storable::Result<()> {
        use storable::*;
        let path = path.as_ref().to_path_buf();
        self.version.store(&path)?;
        store_yaml(&self.info, &path, ANSWER_INFO_FILE)?;
        RawData::store_option(&self.raw_data, &path)?;
        store_object(&self.content, path.clone(), "content")?;
        store_object(&self.comments, path, "comments")
    }
}
has_image!(Answer {
    content: image(),
    comments: image()
});

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
