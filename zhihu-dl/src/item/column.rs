use super::any;
use crate::{
    element::{content::HasContent, Author, Content},
    meta::Version,
    raw_data::{self, FromRaw, RawData},
    store::{BasicStoreContainer, BasicStoreItem, ItemList},
};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, collections::BTreeSet, fmt::Display, str::FromStr};
use web_dl_base::{
    id::{HasId, OwnedId},
    media::{HasImage, Image},
    storable::Storable,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ColumnId(pub String);
impl Display for ColumnId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for ColumnId {
    type Err = <String as FromStr>::Err;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(String::from(s)))
    }
}
impl OwnedId<Column> for ColumnId {
    fn to_id(&self) -> <Column as HasId>::Id<'_> {
        ColumnRef(self.0.as_str())
    }
}
impl Borrow<str> for ColumnId {
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Storable, HasImage, Serialize, Deserialize)]
#[store(format = "yaml")]
pub struct ColumnInfo {
    pub id: ColumnId,
    pub title: String,
    pub author: Author,
    #[has_image]
    pub image: Option<Image>,
    pub created_time: DateTime<FixedOffset>,
    pub updated_time: DateTime<FixedOffset>,
}

const VERSION: Version = Version { major: 1, minor: 0 };
#[derive(Debug, Storable, HasImage, HasContent, Serialize, Deserialize)]
pub struct Column {
    #[store(path(ext = "yaml"))]
    pub version: Version,
    #[has_image(error = "pass_through")]
    #[store(path(ext = "yaml"))]
    pub info: ColumnInfo,
    #[has_image]
    #[content]
    pub intro: Content,
    #[has_image]
    #[content(main)]
    pub description: Content,
    #[store(raw_data)]
    pub raw_data: Option<RawData>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ColumnRef<'a>(pub &'a str);
impl<'a> Display for ColumnRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl HasId for Column {
    const TYPE: &'static str = "column";
    type Id<'a> = ColumnRef<'a>;
    fn id(&self) -> Self::Id<'_> {
        ColumnRef(self.info.id.0.as_str())
    }
}
impl BasicStoreItem for Column {
    fn in_store(
        id: Self::Id<'_>,
        store: &crate::store::ObjectInfo,
    ) -> crate::store::info::ItemInfo {
        store
            .column
            .get(id.0)
            .copied()
            .unwrap_or_default()
            .container
    }
    fn add_info(
        id: Self::Id<'_>,
        info: crate::store::info::ItemInfo,
        store: &mut crate::store::ObjectInfo,
    ) {
        store
            .column
            .entry(ColumnId(id.0.to_owned()))
            .or_default()
            .container = info;
    }
}

impl super::Fetchable for Column {
    async fn fetch<'a>(
        client: &crate::request::Client,
        id: Self::Id<'a>,
    ) -> Result<serde_json::Value, reqwest::Error> {
        client
            .http_client
            .get(format!("https://www.zhihu.com/api/v4/columns/{}", id))
            .query(&[("include", "intro,created")])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }
}

#[derive(Deserialize)]
pub struct Reply {
    id: String,
    title: String,
    author: FromRaw<Author>,
    created: FromRaw<DateTime<FixedOffset>>,
    updated: FromRaw<DateTime<FixedOffset>>,
    image_url: FromRaw<Option<Image>>,
    intro: FromRaw<Content>,
    description: FromRaw<Content>,
}
impl super::Item for Column {
    type Reply = Reply;
    fn from_reply(reply: Self::Reply, raw_data: RawData) -> Self {
        Self {
            version: VERSION,
            info: ColumnInfo {
                id: ColumnId(reply.id),
                title: reply.title,
                author: reply.author.0,
                image: reply.image_url.0,
                created_time: reply.created.0,
                updated_time: reply.updated.0,
            },
            intro: reply.intro.0,
            description: reply.description.0,
            raw_data: Some(raw_data),
        }
    }
    async fn get_images<P: crate::progress::ItemProg>(
        &mut self,
        client: &crate::request::Client,
        prog: &P,
    ) -> bool {
        use crate::progress::ImagesProg;
        let url_i = self.intro.image_urls();
        let url_d = self.description.image_urls();
        let mut prog = prog.start_images((url_i.len() + url_d.len()) as u64 + 1);
        self.intro.fetch_images(client, &mut prog, url_i).await
            | self
                .description
                .fetch_images(client, &mut prog, url_d)
                .await
            | match &mut self.info.image {
                Some(i) => i.fetch(&client.http_client, &mut prog).await,
                None => {
                    prog.skip();
                    false
                }
            }
    }
}
impl ItemList<Column> for BTreeSet<ColumnId> {
    fn insert(&mut self, id: <Column as HasId>::Id<'_>) {
        self.insert(ColumnId(id.0.to_string()));
    }
    fn remove(&mut self, id: <Column as HasId>::Id<'_>) {
        self.remove(id.0);
    }
    fn set_item_info(&self, info: crate::store::info::ItemInfo, store: &mut crate::store::Store) {
        for i in self.iter() {
            store.objects.column.entry(i.clone()).or_default().container = info;
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ColumnItem {
    Regular,
    Pinned,
}
impl Display for ColumnItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Regular => f.write_str("regular"),
            Self::Pinned => f.write_str("pinned"),
        }
    }
}
pub struct Regular;
impl BasicStoreContainer<Regular, super::any::Any> for Column {
    const OPTION_NAME: &'static str = "item";
    type ItemList = any::AnyList;
    fn in_store(id: Self::Id<'_>, store: &crate::store::Store) -> bool {
        store.objects.column.get(id.0).map_or(false, |v| v.item)
    }
    fn add_info(id: Self::Id<'_>, store: &mut crate::store::Store) {
        store
            .objects
            .column
            .entry(ColumnId(id.0.to_string()))
            .or_default()
            .item = true;
    }
}
impl super::ItemContainer<Regular, super::any::Any> for Column {
    async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
        client: &crate::request::Client,
        prog: &P,
        id: Self::Id<'a>,
    ) -> Result<std::collections::LinkedList<RawData>, reqwest::Error> {
        client
            .get_paged::<{ raw_data::Container::Column }, _, _>(
                prog.start_fetch(),
                format!("https://www.zhihu.com/api/v4/columns/{}/items", id,),
            )
            .await
    }
}

pub struct Pinned;
impl BasicStoreContainer<Pinned, super::any::Any> for Column {
    const OPTION_NAME: &'static str = "pinned-item";
    type ItemList = any::AnyList;
    fn in_store(id: Self::Id<'_>, store: &crate::store::Store) -> bool {
        store
            .objects
            .column
            .get(id.0)
            .map_or(false, |v| v.pinned_item)
    }
    fn add_info(id: Self::Id<'_>, store: &mut crate::store::Store) {
        store
            .objects
            .column
            .entry(ColumnId(id.0.to_owned()))
            .or_default()
            .pinned_item = true;
    }
}
impl super::ItemContainer<Pinned, super::any::Any> for Column {
    async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
        client: &crate::request::Client,
        prog: &P,
        id: Self::Id<'a>,
    ) -> Result<std::collections::LinkedList<RawData>, reqwest::Error> {
        client
            .get_paged::<{ raw_data::Container::Column }, _, _>(
                prog.start_fetch(),
                format!("https://www.zhihu.com/api/v4/columns/{}/pinned-items", id),
            )
            .await
    }
}
