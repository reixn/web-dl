use crate::{element::content::HasContent, progress, raw_data::RawData, request::Client, store};
use serde::Deserialize;
use web_dl_base::{id::HasId, media::HasImage};

pub trait Fetchable: HasId {
    async fn fetch<'a>(
        client: &Client,
        id: Self::Id<'a>,
    ) -> Result<serde_json::Value, reqwest::Error>;
}
pub trait Item: Sized + HasId + HasContent + HasImage + store::StoreItem {
    type Reply: for<'de> Deserialize<'de>;
    fn from_reply(reply: Self::Reply, raw_data: RawData) -> Self;
    async fn get_images<P: progress::ItemProg>(&mut self, client: &Client, prog: &P) -> bool;
}

pub trait ItemContainer<O, I: Item>: HasId + store::StoreContainer<O, I> {
    /// there are items on the server, not empty nor deleted
    fn has_item(&self) -> bool {
        true
    }
    #[allow(unused_variables)]
    fn set_info(&self, has_item: bool) {}
    async fn fetch_items<'a, P: progress::ItemContainerProg>(
        client: &Client,
        prog: &P,
        id: Self::Id<'a>,
    ) -> Result<std::collections::LinkedList<RawData>, reqwest::Error>;
    fn parse_item(raw_data: RawData) -> Result<I, serde_json::Error> {
        I::Reply::deserialize(&raw_data.data).map(|r| I::from_reply(r, raw_data))
    }
    #[allow(unused)]
    async fn fixup<'a, P: progress::ItemProg>(
        client: &Client,
        prog: &P,
        id: Self::Id<'a>,
        data: &mut I,
    ) -> Result<bool, reqwest::Error> {
        Ok(false)
    }
}
#[derive(Debug, Clone, Copy)]
pub struct VoidOpt;

#[macro_use]
pub mod comment;
pub use comment::{Comment, CommentId};

pub mod answer;
pub use answer::{Answer, AnswerId};

pub mod any;

pub mod article;
pub use article::{Article, ArticleId};

pub mod collection;
pub use collection::{Collection, CollectionId};

pub mod column;
pub use column::{Column, ColumnId};

pub mod pin;
pub use pin::{Pin, PinId};

pub mod question;
pub use question::{Question, QuestionId};

pub mod user;
pub use user::{User, UserId};

pub mod other;
