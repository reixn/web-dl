use crate::{
    element::{comment::HasComment, content::HasContent},
    item::{
        answer, article,
        other::{OtherInfo, OtherItem},
    },
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

#[derive(Debug, HasImage, HasContent, Serialize, Deserialize)]
pub enum Any {
    Answer(
        #[has_image]
        #[content(main)]
        answer::Answer,
    ),
    Article(
        #[has_image]
        #[content(main)]
        article::Article,
    ),
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
impl HasComment for Any {
    fn has_comment(&self) -> bool {
        match self {
            Any::Answer(a) => a.has_comment(),
            Any::Article(a) => a.has_comment(),
            Any::Other(_) => false,
        }
    }
    fn is_comment_fetched(&self) -> bool {
        match self {
            Any::Answer(a) => a.is_comment_fetched(),
            Any::Article(a) => a.is_comment_fetched(),
            Any::Other(_) => true,
        }
    }
    async fn get_comments<P: crate::progress::CommentTreeProg>(
        &mut self,
        prog: P,
        client: &crate::request::Client,
    ) -> Result<(), crate::element::comment::fetch::Error> {
        match self {
            Any::Answer(a) => a.get_comments(prog, client).await,
            Any::Article(a) => a.get_comments(prog, client).await,
            Any::Other { .. } => Ok(()),
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
