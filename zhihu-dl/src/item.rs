use crate::{
    element::{comment, content::HasContent},
    progress,
    raw_data::RawData,
    request::Client,
    store,
};
use serde::Deserialize;
use web_dl_base::id::HasId;

pub trait Fetchable: HasId {
    async fn fetch<'a>(
        client: &Client,
        id: Self::Id<'a>,
    ) -> Result<serde_json::Value, reqwest::Error>;
}
pub trait Item: Sized + HasId + HasContent {
    type Reply: for<'de> Deserialize<'de>;
    fn from_reply(reply: Self::Reply, raw_data: RawData) -> Self;
    async fn get_images<P: progress::ItemProg>(&mut self, client: &Client, prog: &P) -> bool;
    async fn get_comments<P: progress::ItemProg>(
        &mut self,
        client: &Client,
        prog: &P,
    ) -> Result<(), comment::FetchError>;
}

pub trait ItemContainer<O, I: Item>: HasId + store::StoreItemContainer<O, I> {
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

pub mod answer;
pub use answer::Answer;

pub mod any;

pub mod article;
pub use article::Article;

pub mod collection;
pub use collection::Collection;

pub mod column;
pub use column::Column;

pub mod pin;
pub use pin::Pin;

pub mod question;
pub use question::Question;

pub mod user;
pub use user::User;
