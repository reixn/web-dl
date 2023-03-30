pub use crate::element::author::{UserId, UserType};
use crate::{
    element::Content,
    meta::Version,
    raw_data::{FromRaw, RawData},
};
use serde::{Deserialize, Serialize};
use web_dl_base::{
    id::HasId,
    media::{HasImage, Image},
    storable::Storable,
};

const VERSION: Version = Version { major: 1, minor: 0 };
#[derive(Debug, Storable, HasImage, Serialize, Deserialize)]
#[store(format = "yaml")]
pub struct UserInfo {
    pub id: UserId,
    pub user_type: UserType,
    pub name: String,
    pub url_token: String,
    pub headline: String,
    #[store(has_image)]
    pub avatar: Image,
    #[store(has_image)]
    pub cover: Option<Image>,
}

#[derive(Debug, Storable, HasImage)]
pub struct User {
    #[store(path(ext = "yaml"))]
    pub version: Version,
    #[store(path(ext = "yaml"), has_image(error = "pass_through"))]
    pub info: UserInfo,
    #[store(has_image)]
    pub description: Content,
    #[store(raw_data)]
    pub raw_data: Option<RawData>,
}
impl HasId for User {
    const TYPE: &'static str = "user";
    type Id<'a> = UserId;
    fn id(&self) -> Self::Id<'_> {
        self.info.id
    }
}

impl super::Fetchable for User {
    async fn fetch<'a>(
        client: &crate::request::Client,
        id: Self::Id<'a>,
    ) -> Result<serde_json::Value, reqwest::Error> {
        client
            .http_client
            .get(format!("https://www.zhihu.com/api/v4/members/{}", id))
            .query(&[("include", "description,cover_url")])
            .send()
            .await?
            .json()
            .await
    }
}

#[derive(Deserialize)]
pub struct Reply {
    id: FromRaw<UserId>,
    user_type: FromRaw<UserType>,
    name: String,
    url_token: String,
    headline: String,
    avatar_url: FromRaw<Image>,
    cover_url: FromRaw<Option<Image>>,
    description: FromRaw<Content>,
}
impl super::Item for User {
    type Reply = Reply;
    fn from_reply(reply: Self::Reply, raw_data: RawData) -> Self {
        Self {
            version: VERSION,
            info: UserInfo {
                id: reply.id.0,
                user_type: reply.user_type.0,
                name: reply.name,
                url_token: reply.url_token,
                headline: reply.headline,
                avatar: reply.avatar_url.0,
                cover: reply.cover_url.0,
            },
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
        let mut p = prog.start_images(1 + if self.info.cover.is_some() { 1 } else { 0 });
        self.info.avatar.fetch(&client.http_client, &mut p).await
            | match &mut self.info.cover {
                Some(c) => c.fetch(&client.http_client, &mut p).await,
                None => false,
            }
    }
}
