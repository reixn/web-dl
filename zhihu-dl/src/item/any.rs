use crate::{
    element::content::HasContent,
    item::{answer, article},
    raw_data::RawData,
    store::StoreItem,
};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, path::PathBuf};
use web_dl_base::{id::HasId, media::HasImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnyId<'a> {
    Answer(answer::AnswerId),
    Article(article::ArticleId),
    Other(&'a Option<OtherItem>),
}
impl<'a> Display for AnyId<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnyId::Answer(a) => f.write_fmt(format_args!("answer {}", a)),
            AnyId::Article(a) => f.write_fmt(format_args!("article {}", a)),
            AnyId::Other(Some(v)) => {
                f.write_fmt(format_args!("unknown ({} {})", v.item_type, v.id))
            }
            AnyId::Other(None) => f.write_str("unknown"),
        }
    }
}
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OtherItem {
    pub id: u64,
    #[serde(rename = "type")]
    pub item_type: String,
}

#[derive(Debug, HasImage, Serialize, Deserialize)]
pub enum Any {
    Answer(#[has_image] answer::Answer),
    Article(#[has_image] article::Article),
    Other {
        item: Option<OtherItem>,
        raw_data: RawData,
    },
}
impl HasId for Any {
    const TYPE: &'static str = "any";
    type Id<'a> = AnyId<'a>;
    fn id(&self) -> Self::Id<'_> {
        match self {
            Any::Answer(a) => AnyId::Answer(a.info.id),
            Any::Article(a) => AnyId::Article(a.info.id),
            Any::Other { item, .. } => AnyId::Other(item),
        }
    }
}
impl HasContent for Any {
    fn convert_html(&mut self) {
        match self {
            Any::Answer(a) => a.convert_html(),
            Any::Article(a) => a.convert_html(),
            Any::Other { .. } => (),
        }
    }
}

impl StoreItem for Any {
    fn in_store(id: Self::Id<'_>, store: &crate::store::Store) -> bool {
        match id {
            AnyId::Answer(a) => answer::Answer::in_store(a, store),
            AnyId::Article(a) => article::Article::in_store(a, store),
            AnyId::Other(_) => false,
        }
    }
    fn link_info(
        id: Self::Id<'_>,
        store: &crate::store::Store,
        dest: Option<PathBuf>,
    ) -> Option<crate::store::LinkInfo> {
        match id {
            AnyId::Answer(a) => answer::Answer::link_info(a, store, dest),
            AnyId::Article(a) => article::Article::link_info(a, store, dest),
            AnyId::Other(i) => {
                match i {
                    Some(it) => {
                        log::warn!("skipped unrecognized object ({} {})", it.item_type, it.id)
                    }
                    None => log::warn!("skipped unrecognized object (unknown id,type)"),
                }
                None
            }
        }
    }
    fn save_data(
        &self,
        store: &mut crate::store::Store,
        dest: Option<PathBuf>,
    ) -> Result<Option<crate::store::LinkInfo>, web_dl_base::storable::Error> {
        match self {
            Any::Answer(a) => a.save_data(store, dest),
            Any::Article(a) => a.save_data(store, dest),
            Any::Other { item, raw_data } => {
                match item {
                    Some(it) => {
                        log::warn!(
                            "skipped storing unrecognized object ({} {})",
                            it.item_type,
                            it.id
                        )
                    }
                    None => log::warn!("skipped storing unrecognized object (unknown id,type)"),
                }
                log::trace!("ignored object raw data: {:#?}", raw_data);
                Ok(None)
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum Reply {
    #[serde(rename = "answer")]
    Answer(answer::Reply),
    #[serde(rename = "article")]
    Article(article::Reply),
    #[serde(other)]
    Other,
}
impl super::Item for Any {
    type Reply = Reply;
    fn from_reply(reply: Self::Reply, raw_data: RawData) -> Self {
        match reply {
            Reply::Answer(a) => Any::Answer(answer::Answer::from_reply(a, raw_data)),
            Reply::Article(a) => Any::Article(article::Article::from_reply(a, raw_data)),
            Reply::Other => Any::Other {
                item: OtherItem::deserialize(&raw_data.data).ok(),
                raw_data,
            },
        }
    }
    async fn get_comments<P: crate::progress::ItemProg>(
        &mut self,
        client: &crate::request::Client,
        prog: &P,
    ) -> Result<(), crate::element::comment::FetchError> {
        match self {
            Any::Answer(a) => a.get_comments(client, prog).await,
            Any::Article(a) => a.get_comments(client, prog).await,
            Any::Other { .. } => Ok(()),
        }
    }
    async fn get_images<P: crate::progress::ItemProg>(
        &mut self,
        client: &crate::request::Client,
        prog: &P,
    ) -> bool {
        match self {
            Any::Answer(a) => a.get_images(client, prog).await,
            Any::Article(a) => a.get_images(client, prog).await,
            Any::Other { .. } => false,
        }
    }
}
