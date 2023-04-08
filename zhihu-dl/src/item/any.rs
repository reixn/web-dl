use crate::{
    element::content::HasContent,
    item::{answer, article},
    raw_data::RawData,
    store::StoreItem,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fmt::Display,
    path::{Path, PathBuf},
};
use web_dl_base::{id::HasId, media::HasImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum AnyId<'a> {
    Answer(answer::AnswerId),
    Article(article::ArticleId),
    Other(&'a OtherItem),
}
impl<'a> Display for AnyId<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnyId::Answer(a) => f.write_fmt(format_args!("answer {}", a)),
            AnyId::Article(a) => f.write_fmt(format_args!("article {}", a)),
            AnyId::Other(OtherItem { info: Some(v), .. }) => {
                f.write_fmt(format_args!("unknown ({} {})", v.item_type, v.id))
            }
            AnyId::Other(OtherItem { info: None, .. }) => f.write_str("unknown"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OtherInfo {
    pub id: u64,
    #[serde(rename = "type")]
    pub item_type: String,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtherItem {
    pub info: Option<OtherInfo>,
    pub raw_data: RawData,
}
impl PartialOrd for OtherItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering;
        match self.info.cmp(&other.info) {
            Ordering::Equal => {
                if self.raw_data == other.raw_data {
                    Some(Ordering::Equal)
                } else {
                    None
                }
            }
            a => Some(a),
        }
    }
}
impl OtherItem {
    pub(crate) fn warn(&self) {
        match &self.info {
            Some(it) => {
                log::warn!("skipped unrecognized object ({} {})", it.item_type, it.id)
            }
            None => log::warn!("skipped unrecognized object (unknown id,type)"),
        }
        log::trace!("ignored unknown object: {:#?}", self.raw_data);
    }
}

#[derive(Debug, HasImage, Serialize, Deserialize)]
pub enum Any {
    Answer(#[has_image] answer::Answer),
    Article(#[has_image] article::Article),
    Other(OtherItem),
}
impl HasId for Any {
    const TYPE: &'static str = "any";
    type Id<'a> = AnyId<'a>;
    fn id(&self) -> Self::Id<'_> {
        match self {
            Any::Answer(a) => AnyId::Answer(a.info.id),
            Any::Article(a) => AnyId::Article(a.info.id),
            Any::Other(item) => AnyId::Other(item),
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
    fn get_main_content(&self) -> Option<&'_ crate::element::Content> {
        match self {
            Any::Answer(a) => a.get_main_content(),
            Any::Article(a) => a.get_main_content(),
            Any::Other { .. } => None,
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
    fn link_info<P: AsRef<Path>>(
        id: Self::Id<'_>,
        store: &crate::store::Store,
        dest: P,
    ) -> Option<crate::store::LinkInfo> {
        match id {
            AnyId::Answer(a) => answer::Answer::link_info(a, store, dest),
            AnyId::Article(a) => article::Article::link_info(a, store, dest),
            AnyId::Other(i) => {
                i.warn();
                None
            }
        }
    }
    fn save_data(
        &self,
        store: &mut crate::store::Store,
    ) -> Result<Option<PathBuf>, web_dl_base::storable::Error> {
        match self {
            Any::Answer(a) => a.save_data(store),
            Any::Article(a) => a.save_data(store),
            Any::Other(o) => {
                o.warn();
                Ok(None)
            }
        }
    }
    fn save_data_link<P: AsRef<Path>>(
        &self,
        store: &mut crate::store::Store,
        dest: P,
    ) -> Result<Option<crate::store::LinkInfo>, web_dl_base::storable::Error> {
        match self {
            Any::Answer(a) => a.save_data_link(store, dest),
            Any::Article(a) => a.save_data_link(store, dest),
            Any::Other(o) => {
                o.warn();
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
            Reply::Other => Any::Other(OtherItem {
                info: OtherInfo::deserialize(&raw_data.data).ok(),
                raw_data,
            }),
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

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AnyList {
    pub answer: BTreeSet<answer::AnswerId>,
    pub article: BTreeSet<article::ArticleId>,
}
impl AnyList {
    pub fn insert(&mut self, id: AnyId) {
        match id {
            AnyId::Answer(a) => {
                self.answer.insert(a);
            }
            AnyId::Article(a) => {
                self.article.insert(a);
            }
            AnyId::Other { .. } => (),
        }
    }
}
