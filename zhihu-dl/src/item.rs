use crate::{
    element::{comment, content::HasContent},
    progress,
    raw_data::RawData,
    request::Client,
};
use serde::Deserialize;
use std::fmt::Display;
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

pub trait ItemContainer<I: Item, O: Display + Copy>: HasId {
    async fn fetch_items<'a, P: progress::ItemContainerProg>(
        client: &Client,
        prog: &P,
        id: Self::Id<'a>,
        option: O,
    ) -> Result<std::collections::LinkedList<RawData>, reqwest::Error>;
    fn parse_item(raw_data: RawData) -> Result<I, serde_json::Error> {
        I::Reply::deserialize(&raw_data.data).map(|r| I::from_reply(r, raw_data))
    }
    #[allow(unused)]
    async fn fixup<'a, P: progress::ItemProg>(
        client: &Client,
        prog: &P,
        id: Self::Id<'a>,
        option: O,
        data: &mut I,
    ) -> Result<bool, reqwest::Error> {
        Ok(false)
    }
}
#[derive(Debug, Clone, Copy)]
pub struct VoidOpt;
impl Display for VoidOpt {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

pub mod answer;
pub mod any;
pub mod article;
pub mod collection;
pub mod column;
pub mod pin;
pub mod question;
pub mod user;
