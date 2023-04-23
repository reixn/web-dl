use crate::{
    element::content::HasContent,
    item::{
        answer, article, comment,
        other::{OtherInfo, OtherItem},
    },
    raw_data::RawData,
    store::{self, ItemList, StoreContainer, StoreItem},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fmt::Display,
    path::{Path, PathBuf},
};
use web_dl_base::id::HasId;

use super::ItemContainer;

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

#[derive(Debug, HasContent, Serialize, Deserialize)]
pub enum Any {
    Answer(#[content(main)] answer::Answer),
    Article(#[content(main)] article::Article),
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

impl StoreItem for Any {
    fn in_store(id: Self::Id<'_>, store: &store::Store) -> store::info::ItemInfo {
        match id {
            AnyId::Answer(a) => <answer::Answer as StoreItem>::in_store(a, store),
            AnyId::Article(a) => <article::Article as StoreItem>::in_store(a, store),
            AnyId::Other(_) => store::info::ItemInfo::default(),
        }
    }
    fn add_info(id: Self::Id<'_>, info: store::info::ItemInfo, store: &mut store::Store) {
        match id {
            AnyId::Answer(a) => answer::Answer::add_info(a, info, store),
            AnyId::Article(a) => article::Article::add_info(a, info, store),
            AnyId::Other(_) => (),
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
    fn add_media(&self, store: &mut store::Store) -> Result<(), web_dl_base::media::Error> {
        match self {
            Any::Answer(a) => a.add_media(store),
            Any::Article(a) => a.add_media(store),
            Any::Other(_) => Ok(()),
        }
    }
    fn save_data(
        &self,
        on_server: bool,
        store: &mut crate::store::Store,
    ) -> Result<Option<PathBuf>, web_dl_base::storable::Error> {
        match self {
            Any::Answer(a) => a.save_data(on_server, store),
            Any::Article(a) => a.save_data(on_server, store),
            Any::Other(o) => {
                o.warn();
                Ok(None)
            }
        }
    }
    fn save_data_link<P: AsRef<Path>>(
        &self,
        on_server: bool,
        store: &mut crate::store::Store,
        dest: P,
    ) -> Result<Option<crate::store::LinkInfo>, web_dl_base::storable::Error> {
        match self {
            Any::Answer(a) => a.save_data_link(on_server, store, dest),
            Any::Article(a) => a.save_data_link(on_server, store, dest),
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
impl ItemList<Any> for AnyList {
    fn insert(&mut self, id: <Any as HasId>::Id<'_>) {
        match id {
            AnyId::Answer(a) => {
                self.answer.insert(a);
            }
            AnyId::Article(a) => {
                self.article.insert(a);
            }
            AnyId::Other(_) => (),
        }
    }
    fn remove(&mut self, id: <Any as HasId>::Id<'_>) {
        match id {
            AnyId::Answer(a) => {
                self.answer.remove(&a);
            }
            AnyId::Article(a) => {
                self.article.remove(&a);
            }
            AnyId::Other(_) => (),
        }
    }
    fn set_item_info(&self, info: store::info::ItemInfo, store: &mut store::Store) {
        self.answer.set_item_info(info, store);
        self.article.set_item_info(info, store);
    }
}

pub enum AnyContainer<'a, 'b> {
    Answer(<answer::Answer as StoreContainer<super::VoidOpt, comment::Comment>>::Handle<'a, 'b>),
    Article(<article::Article as StoreContainer<super::VoidOpt, comment::Comment>>::Handle<'a, 'b>),
    Other,
}
impl<'a, 'b> store::ContainerHandle<comment::Comment> for AnyContainer<'a, 'b> {
    fn link_item(
        &mut self,
        id: <comment::Comment as HasId>::Id<'_>,
    ) -> Result<(), store::StoreError> {
        match self {
            Self::Answer(a) => a.link_item(id),
            Self::Article(a) => a.link_item(id),
            Self::Other => Ok(()),
        }
    }
    fn mark_missing(&mut self) {
        match self {
            Self::Answer(a) => a.mark_missing(),
            Self::Article(a) => a.mark_missing(),
            Self::Other => (),
        }
    }
    fn finish(self) -> Result<Option<PathBuf>, store::StoreError> {
        match self {
            Self::Answer(a) => a.finish(),
            Self::Article(a) => a.finish(),
            Self::Other => Ok(None),
        }
    }
}
impl StoreContainer<super::VoidOpt, comment::Comment> for Any {
    const OPTION_NAME: &'static str = "comment";
    fn in_store(id: Self::Id<'_>, store: &crate::store::Store) -> bool {
        match id {
            AnyId::Answer(a) => <answer::Answer as StoreContainer<
                super::VoidOpt,
                comment::Comment,
            >>::in_store(a, store),
            AnyId::Article(a) => <article::Article as StoreContainer<
                super::VoidOpt,
                comment::Comment,
            >>::in_store(a, store),
            AnyId::Other(_) => true,
        }
    }
    fn store_path(id: Self::Id<'_>, store: &store::Store) -> Option<PathBuf> {
        match id {
            AnyId::Answer(a) => <answer::Answer as StoreContainer<
                super::VoidOpt,
                comment::Comment,
            >>::store_path(a, store),
            AnyId::Article(a) => <article::Article as StoreContainer<
                super::VoidOpt,
                comment::Comment,
            >>::store_path(a, store),
            AnyId::Other(_) => None,
        }
    }
    type Handle<'a, 'b> = AnyContainer<'a, 'b>;
    fn save_data<'a, 'b>(
        id: Self::Id<'a>,
        store: &'b mut store::Store,
    ) -> Result<Self::Handle<'a, 'b>, store::StoreError> {
        Ok(match id {
            AnyId::Answer(a) => AnyContainer::Answer(<answer::Answer as StoreContainer<
                super::VoidOpt,
                comment::Comment,
            >>::save_data(a, store)?),
            AnyId::Article(a) => AnyContainer::Article(<article::Article as StoreContainer<
                super::VoidOpt,
                comment::Comment,
            >>::save_data(a, store)?),
            AnyId::Other(_) => AnyContainer::Other,
        })
    }
}
impl ItemContainer<super::VoidOpt, comment::Comment> for Any {
    fn has_item(&self) -> bool {
        match self {
            Self::Answer(a) => a.has_item(),
            Self::Article(a) => a.has_item(),
            Self::Other(_) => false,
        }
    }
    fn set_info(&self, has_item: bool) {
        match self {
            Self::Answer(a) => a.set_info(has_item),
            Self::Article(a) => a.set_info(has_item),
            Self::Other(_) => (),
        }
    }
    async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
        client: &crate::request::Client,
        prog: &P,
        id: Self::Id<'a>,
    ) -> Result<std::collections::LinkedList<RawData>, reqwest::Error> {
        match id {
            AnyId::Answer(a) => {
                <answer::Answer as ItemContainer<super::VoidOpt, comment::Comment>>::fetch_items(
                    client, prog, a,
                )
                .await
            }
            AnyId::Article(a) => {
                <article::Article as ItemContainer<super::VoidOpt, comment::Comment>>::fetch_items(
                    client, prog, a,
                )
                .await
            }
            AnyId::Other(_) => Ok(Default::default()),
        }
    }
}
