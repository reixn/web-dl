use crate::{
    element::{
        author::Author,
        content::{Content, HasContent},
    },
    meta::Version,
    progress,
    raw_data::{self, FromRaw, RawData, StrU64},
    request::Client,
    store::BasicStoreItem,
};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::{cell::Cell, fmt::Display, str::FromStr};
use web_dl_base::{id::HasId, media::StoreImage, storable::Storable};

pub const VERSION: Version = Version { major: 2, minor: 1 };

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CommentId(pub u64);
impl Display for CommentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for CommentId {
    type Err = <u64 as FromStr>::Err;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        u64::from_str(s).map(Self)
    }
}
impl<'de> Deserialize<'de> for FromRaw<CommentId> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        StrU64::deserialize(deserializer).map(|v| FromRaw(CommentId(v.0)))
    }
}

#[derive(Debug, Storable, Serialize, Deserialize)]
#[store(format = "yaml")]
pub struct CommentInfo {
    pub id: CommentId,
    pub parent_id: Option<CommentId>,
    pub author: Option<Author>,
    pub is_author: bool,
    pub has_child: Cell<bool>,
    pub created_time: DateTime<FixedOffset>,
}

#[derive(Debug, Storable, HasContent, StoreImage, Serialize, Deserialize)]
pub struct Comment {
    #[store(path(ext = "yaml"))]
    pub version: Version,
    #[store(path(ext = "yaml"))]
    pub info: CommentInfo,
    #[has_image]
    #[content(main)]
    pub content: Content,
    #[store(raw_data)]
    pub raw_data: Option<RawData>,
}

impl HasId for Comment {
    const TYPE: &'static str = "comment";
    type Id<'a> = CommentId;
    fn id(&self) -> Self::Id<'_> {
        self.info.id
    }
}
basic_store_item!(Comment, comment);

#[derive(Deserialize)]
pub struct Reply {
    id: FromRaw<CommentId>,
    reply_comment_id: FromRaw<CommentId>,
    author: FromRaw<Option<Author>>,
    is_author: bool,
    child_comment_count: u32,
    created_time: FromRaw<DateTime<FixedOffset>>,
    #[serde(default)]
    content: FromRaw<Content>,
}
impl super::Item for Comment {
    type Reply = Reply;
    fn from_reply(reply: Self::Reply, raw_data: RawData) -> Self {
        Self {
            version: VERSION,
            info: CommentInfo {
                id: reply.id.0,
                parent_id: if reply.reply_comment_id.0 .0 == 0 {
                    None
                } else {
                    Some(reply.reply_comment_id.0)
                },
                author: reply.author.0,
                is_author: reply.is_author,
                has_child: Cell::new(reply.child_comment_count > 0),
                created_time: reply.created_time.0,
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
item_list_btree!(Comment, CommentId);

macro_rules! comment_store_container {
    ($t:ty, $i:ident) => {
        impl crate::store::BasicStoreContainer<super::VoidOpt, crate::item::comment::Comment>
            for $t
        {
            const OPTION_NAME: &'static str = "comment";
            type ItemList = std::collections::BTreeSet<crate::item::comment::CommentId>;
            fn in_store(id: Self::Id<'_>, store: &crate::store::Store) -> bool {
                store.objects.$i.get(&id).map_or(false, |v| v.comment)
            }
            fn add_info(id: Self::Id<'_>, store: &mut crate::store::Store) {
                store.objects.$i.entry(id).or_default().comment = true;
            }
        }
    };
}

comment_store_container!(Comment, comment);
impl super::ItemContainer<super::VoidOpt, Comment> for Comment {
    fn has_item(&self) -> bool {
        self.info.has_child.get()
    }
    fn set_info(&self, has_item: bool) {
        self.info.has_child.set(has_item)
    }
    async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
        client: &crate::request::Client,
        prog: &P,
        id: Self::Id<'a>,
    ) -> Result<std::collections::LinkedList<RawData>, reqwest::Error> {
        client
            .get_paged::<{ raw_data::Container::None }, _, _>(
                prog.start_fetch(),
                format!(
                    "https://www.zhihu.com/api/v4/comment_v5/comment/{}/child_comment",
                    id
                ),
            )
            .await
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RootType {
    Article,
    Answer,
    Collection,
    Pin,
    Question,
}
pub async fn fetch_root<I: Display, P: progress::FetchProg>(
    client: &Client,
    prog: P,
    root_type: RootType,
    id: I,
) -> Result<std::collections::LinkedList<RawData>, reqwest::Error> {
    client
        .get_paged::<{ raw_data::Container::None }, _, _>(
            prog,
            format!(
                "https://www.zhihu.com/api/v4/comment_v5/{}/{}/root_comment",
                match root_type {
                    RootType::Answer => "answers",
                    RootType::Article => "articles",
                    RootType::Collection => "collections",
                    RootType::Pin => "pins",
                    RootType::Question => "questions",
                },
                id
            ),
        )
        .await
}
macro_rules! comment_container {
    ($t:ident, $($i:ident).+) => {
        impl crate::item::ItemContainer<crate::item::VoidOpt, crate::item::comment::Comment> for $t {
            fn has_item(&self) -> bool {
                self$(.$i)+.get()
            }
            fn set_info(&self, has_item: bool) {
                self$(.$i)+.set(has_item)
            }
            async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
                client: &crate::request::Client,
                prog: &P,
                id: Self::Id<'a>,
            ) -> Result<std::collections::LinkedList<crate::raw_data::RawData>, reqwest::Error> {
                crate::item::comment::fetch_root(
                    client,
                    prog.start_fetch(),
                    crate::item::comment::RootType::$t,
                    id
                )
                .await
            }
        }
    };
}

#[inline]
pub(crate) fn has_comment_default() -> Cell<bool> {
    Cell::new(true)
}
