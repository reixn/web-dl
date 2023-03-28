use crate::{
    element::comment, id::HasId, progress, raw_data::RawData, request::Client,
    store::storable::Storable,
};
use serde::Deserialize;
use std::{error, fmt::Display};

#[derive(Debug)]
pub enum ErrorSource {
    Http(reqwest::Error),
    Json(serde_json::Error),
    Comment(comment::FetchError),
}
#[derive(Debug)]
pub struct Error {
    item_kind: &'static str,
    item_id: String,
    source: ErrorSource,
}
impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "failed to {} when processing {} {}",
            match self.source {
                ErrorSource::Http(_) => "fetch data",
                ErrorSource::Json(_) => "parse response",
                ErrorSource::Comment(_) => "get comment",
            },
            self.item_kind,
            self.item_id
        ))
    }
}
impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self.source {
            ErrorSource::Http(e) => Some(e),
            ErrorSource::Json(e) => Some(e),
            ErrorSource::Comment(e) => Some(e),
        }
    }
}

pub trait Fetchable: HasId {
    async fn fetch<'a>(
        client: &Client,
        id: Self::Id<'a>,
    ) -> Result<serde_json::Value, reqwest::Error>;
}
pub trait Item: Sized + HasId + Storable {
    type Reply: for<'de> Deserialize<'de>;
    fn from_reply(reply: Self::Reply, raw_data: RawData) -> Self;
    async fn get_images<P: progress::ItemProg>(&mut self, client: &Client, prog: &P) -> bool;
    async fn get_comments<P: progress::ItemProg>(
        &mut self,
        client: &Client,
        prog: &P,
    ) -> Result<(), comment::FetchError>;
}

pub async fn get_item<'a, const COM: bool, I: Fetchable + Item, P: progress::ItemProg>(
    client: &Client,
    mut prog: P,
    id: I::Id<'a>,
) -> Result<I, Error> {
    log::debug!("fetching {} {}", I::TYPE, id);
    let data = I::fetch(client, id).await.map_err(|e| Error {
        item_kind: I::TYPE,
        item_id: id.to_string(),
        source: ErrorSource::Http(e),
    })?;
    log::debug!("parsing raw data");
    log::trace!("raw data {:#?}", data);
    let mut item = I::from_reply(
        I::Reply::deserialize(&data).map_err(|e| Error {
            item_kind: I::TYPE,
            item_id: id.to_string(),
            source: ErrorSource::Json(e),
        })?,
        RawData {
            data,
            fetch_time: chrono::Utc::now(),
        },
    );
    log::debug!("fetching images");
    item.get_images(client, &mut prog).await;
    if COM {
        log::debug!("fetching comments");
        item.get_comments(client, &mut prog)
            .await
            .map_err(|e| Error {
                item_kind: I::TYPE,
                item_id: id.to_string(),
                source: ErrorSource::Comment(e),
            })?;
    }
    Ok(item)
}

pub mod answer;
pub mod article;
pub mod collection;
pub mod column;
pub mod pin;
pub mod question;
pub mod user;
