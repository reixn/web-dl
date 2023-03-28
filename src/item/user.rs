pub use crate::element::author::{UserId, UserType};
use crate::{
    element::{Content, Image},
    id,
    meta::Version,
    raw_data::{FromRaw, RawData},
    store::storable,
};
use serde::{Deserialize, Serialize};

const VERSION: Version = Version { major: 1, minor: 0 };
#[derive(Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: UserId,
    pub user_type: UserType,
    pub name: String,
    pub url_token: String,
    pub headline: String,
    pub avatar: Image,
    pub cover: Option<Image>,
}

#[derive(Debug)]
pub struct User {
    pub version: Version,
    pub info: UserInfo,
    pub description: Content,
    pub raw_data: Option<RawData>,
}
impl id::HasId for User {
    const TYPE: &'static str = "user";
    type Id<'a> = UserId;
    fn id(&self) -> Self::Id<'_> {
        self.info.id
    }
}

const USER_INFO_FILE: &str = "user_info.yaml";
impl storable::Storable for User {
    fn load<P: AsRef<std::path::Path>>(
        path: P,
        load_opt: storable::LoadOpt,
    ) -> storable::Result<Self> {
        use storable::*;
        let path = path.as_ref().to_path_buf();
        Ok(Self {
            version: Version::load(&path)?,
            info: {
                let mut info: UserInfo = load_yaml(&path, USER_INFO_FILE)?;
                if load_opt.load_img {
                    info.avatar.load_data(&path, "avatar")?;
                    match &mut info.cover {
                        Some(c) => c.load_data(&path, "cover")?,
                        None => (),
                    }
                }
                info
            },
            raw_data: RawData::load_if(&path, load_opt)?,
            description: load_fixed_id_obj(path, load_opt, "description")?,
        })
    }
    fn store<P: AsRef<std::path::Path>>(&self, path: P) -> storable::Result<()> {
        use storable::*;
        let path = path.as_ref().to_path_buf();
        self.version.store(&path)?;
        store_yaml(&self.info, &path, USER_INFO_FILE)?;
        self.info.avatar.store_data(&path, "avatar")?;
        match &self.info.cover {
            Some(c) => c.store_data(&path, "cover")?,
            None => (),
        }
        RawData::store_option(&self.raw_data, &path)?;
        store_object(&self.description, path, "description")
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
        self.info.avatar.fetch(client, &mut p).await
            | match &mut self.info.cover {
                Some(c) => c.fetch(client, &mut p).await,
                None => false,
            }
    }
}
