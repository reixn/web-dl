pub use crate::element::author::{UserId, UserType};
use crate::{
    element::{content::HasContent, Content},
    item::Item,
    meta::Version,
    progress,
    raw_data::{self, FromRaw, RawData},
    request::Zse96V3,
    store::BasicStoreItem,
};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
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
    #[has_image]
    pub avatar: Image,
    #[has_image]
    pub cover: Option<Image>,
}

#[derive(Debug, Storable, HasImage, Serialize, Deserialize)]
pub struct User {
    #[store(path(ext = "yaml"))]
    pub version: Version,
    #[has_image(error = "pass_through")]
    #[store(path(ext = "yaml"))]
    pub info: UserInfo,
    #[has_image]
    pub description: Content,
    #[store(raw_data)]
    pub raw_data: Option<RawData>,
}

#[derive(Debug, Clone, Copy)]
pub struct StoreId<'a>(pub UserId, pub &'a str);
impl<'a> Display for StoreId<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl HasId for User {
    const TYPE: &'static str = "user";
    type Id<'a> = StoreId<'a>;
    fn id(&self) -> Self::Id<'_> {
        StoreId(self.info.id, self.info.url_token.as_str())
    }
}
impl BasicStoreItem for User {
    fn in_store(id: Self::Id<'_>, info: &crate::store::StoreObject) -> bool {
        info.user.contains(&id.0)
    }
    fn add_info(&self, info: &mut crate::store::StoreObject) {
        info.user.insert(self.info.id);
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
impl HasContent for User {
    fn convert_html(&mut self) {
        self.description.convert_html();
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

mod param;

impl super::ItemContainer<super::answer::Answer, super::VoidOpt> for User {
    async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
        client: &crate::request::Client,
        prog: &P,
        id: Self::Id<'a>,
        _: super::VoidOpt,
    ) -> Result<std::collections::LinkedList<RawData>, reqwest::Error> {
        client
            .get_paged_sign::<{ raw_data::Container::User }, Zse96V3, _, _>(
                prog.start_fetch(),
                Url::parse_with_params(
                    format!("https://www.zhihu.com/api/v4/members/{}/answers", id.1).as_str(),
                    &[("include", param::ANSWER_INCLUDE)],
                )
                .unwrap(),
            )
            .await
    }
}
impl super::ItemContainer<super::article::Article, super::VoidOpt> for User {
    async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
        client: &crate::request::Client,
        prog: &P,
        id: Self::Id<'a>,
        _: super::VoidOpt,
    ) -> Result<std::collections::LinkedList<RawData>, reqwest::Error> {
        client
            .get_paged_sign::<{ raw_data::Container::User }, Zse96V3, _, _>(
                prog.start_fetch(),
                Url::parse_with_params(
                    format!("https://www.zhihu.com/api/v4/members/{}/articles", id.1).as_str(),
                    &[("include", param::ARTICLE_INCLUDE)],
                )
                .unwrap(),
            )
            .await
    }
    async fn fixup<'a, P: progress::ItemProg>(
        client: &crate::request::Client,
        prog: &P,
        _: Self::Id<'a>,
        _: super::VoidOpt,
        data: &mut super::article::Article,
    ) -> Result<bool, reqwest::Error> {
        data.fix_cover(client, prog).await.map(|_| true)
    }
}

impl<'b> super::ItemContainer<super::column::Column, super::VoidOpt> for User {
    async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
        client: &crate::request::Client,
        prog: &P,
        id: Self::Id<'a>,
        _: super::VoidOpt,
    ) -> Result<std::collections::LinkedList<RawData>, reqwest::Error> {
        client
            .get_paged::<{ raw_data::Container::User }, _, _>(
                prog.start_fetch(),
                Url::parse_with_params(
                    format!(
                        "https://www.zhihu.com/api/v4/members/{}/column-contributions",
                        id.1
                    )
                    .as_str(),
                    &[("include", param::COLUMN_INCLUDE)],
                )
                .unwrap(),
            )
            .await
    }
    fn parse_item(raw_data: RawData) -> Result<super::column::Column, serde_json::Error> {
        #[derive(Deserialize)]
        struct Reply {
            column: super::column::Reply,
        }
        Reply::deserialize(&raw_data.data)
            .map(|r| super::column::Column::from_reply(r.column, raw_data))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CollectionOpt {
    Created,
    Liked,
}
impl Display for CollectionOpt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Created => f.write_str("created"),
            Self::Liked => f.write_str("liked"),
        }
    }
}
impl super::ItemContainer<super::collection::Collection, CollectionOpt> for User {
    async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
        client: &crate::request::Client,
        prog: &P,
        id: Self::Id<'a>,
        option: CollectionOpt,
    ) -> Result<std::collections::LinkedList<RawData>, reqwest::Error> {
        client
            .get_paged::<{ raw_data::Container::User }, _, _>(
                prog.start_fetch(),
                match option {
                    CollectionOpt::Created => Url::parse_with_params(
                        format!("https://www.zhihu.com/api/v4/people/{}/collections", id.1)
                            .as_str(),
                        &[("include", param::CREATED_COLL_INCLUDE)],
                    )
                    .unwrap(),
                    CollectionOpt::Liked => Url::parse_with_params(
                        format!(
                            "https://www.zhihu.com/api/v4/members/{}/following-favlists",
                            id.1
                        )
                        .as_str(),
                        &[("include", param::LIKED_COLL_INCLUDE)],
                    )
                    .unwrap(),
                },
            )
            .await
    }
}

impl super::ItemContainer<super::pin::Pin, super::VoidOpt> for User {
    async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
        client: &crate::request::Client,
        prog: &P,
        id: Self::Id<'a>,
        _: super::VoidOpt,
    ) -> Result<std::collections::LinkedList<RawData>, reqwest::Error> {
        client
            .get_paged::<{ raw_data::Container::User }, _, _>(
                prog.start_fetch(),
                format!("https://www.zhihu.com/api/v4/v2/pins/{}/moments", id.1),
            )
            .await
    }
}

mod activity;
pub use activity::{ActTarget, Activity, ActivityId};
