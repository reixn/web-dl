use crate::{
    item::{
        answer::{self, Answer},
        any::OtherItem,
        article::{self, Article},
        collection::{self, Collection},
        column::{self, Column},
        pin::{self, Pin},
        question::{self, Question},
        Item, ItemContainer, VoidOpt,
    },
    raw_data::{self, RawData, StrU64},
    request::Zse96V3,
    store::{LinkInfo, Store, StoreItem},
};
use serde::Deserialize;
use std::{fmt::Display, path::PathBuf};
use web_dl_base::{id::HasId, media::HasImage};

type Id<'a, S> = <S as HasId>::Id<'a>;

#[derive(Debug, Clone, Copy)]
pub enum ActTargetId<'a> {
    Answer(Id<'a, answer::Answer>),
    Article(Id<'a, article::Article>),
    Collection(Id<'a, collection::Collection>),
    Column(Id<'a, column::Column>),
    Pin(Id<'a, pin::Pin>),
    Question(Id<'a, question::Question>),
    Other(&'a Option<OtherItem>),
}

#[derive(Debug, Clone, Copy)]
pub struct ActivityId<'a> {
    id: u64,
    target: ActTargetId<'a>,
}
impl<'a> Display for ActivityId<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ActTargetId::*;
        match self.target {
            Answer(a) => f.write_fmt(format_args!("{} (answer {})", self.id, a)),
            Article(a) => f.write_fmt(format_args!("{} (article {})", self.id, a)),
            Collection(c) => f.write_fmt(format_args!("{} (collection {})", self.id, c)),
            Column(c) => f.write_fmt(format_args!("{} (column {})", self.id, c)),
            Pin(p) => f.write_fmt(format_args!("{} (pin {})", self.id, p)),
            Question(q) => f.write_fmt(format_args!("{} (question {})", self.id, q)),
            Other(Some(i)) => f.write_fmt(format_args!("{} ({} {})", self.id, i.item_type, i.id)),
            Other(None) => f.write_fmt(format_args!("{} (unknown)", self.id)),
        }
    }
}

#[derive(Debug, HasImage)]
pub enum ActTarget {
    Answer(#[has_image] Answer),
    Article(#[has_image] Article),
    Collection(#[has_image] Collection),
    Column(#[has_image] Column),
    Pin(#[has_image] Pin),
    Question(#[has_image] Question),
    Other {
        item: Option<OtherItem>,
        raw_data: RawData,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum TargetReply {
    #[serde(rename = "answer")]
    Answer(answer::Reply),
    #[serde(rename = "article")]
    Article(article::Reply),
    #[serde(rename = "collection")]
    Collection(<Collection as Item>::Reply),
    #[serde(rename = "column")]
    Column(column::Reply),
    #[serde(rename = "pin")]
    Pin(pin::Reply),
    #[serde(rename = "question")]
    Question(question::Reply),
    #[serde(other)]
    Other,
}
#[derive(Deserialize)]
pub struct Reply {
    id: StrU64,
    target: TargetReply,
}

#[derive(Debug, HasImage)]
pub struct Activity {
    pub id: u64,
    #[has_image(error = "pass_through")]
    pub target: ActTarget,
}

impl HasId for Activity {
    const TYPE: &'static str = "activity";
    type Id<'a> = ActivityId<'a>;
    fn id(&self) -> Self::Id<'_> {
        macro_rules! target {
            ($($t:ident),+) => {
                match &self.target {
                    $(ActTarget::$t(t) => ActivityId { id: self.id, target: ActTargetId::$t(t.id()) },)+
                    ActTarget::Other { item,.. } => ActivityId {id: self.id, target: ActTargetId::Other(item)}
                }
            };
        }
        return target!(Answer, Article, Collection, Column, Pin, Question);
    }
}
impl StoreItem for Activity {
    fn in_store<'a>(id: Self::Id<'a>, store: &crate::store::Store) -> bool {
        macro_rules! target {
            ($($t:ident),+) => {
                match id.target {
                    $(ActTargetId::$t(t) => $t::in_store(t, store),)+
                    ActTargetId::Other(_) => false
                }
            };
        }
        return target!(Answer, Article, Collection, Column, Question, Pin);
    }
    fn link_info<'a>(id: Self::Id<'a>, store: &Store, dest: PathBuf) -> Option<LinkInfo> {
        macro_rules! target {
            ($($t:ident),+) => {
                match id.target {
                    $(ActTargetId::$t(t) => $t::link_info(t, store, dest),)+
                    ActTargetId::Other(_) => None
                }
            };
        }
        return target!(Answer, Article, Collection, Column, Question, Pin);
    }
    fn save_data(
        &self,
        store: &mut crate::store::Store,
        dest: PathBuf,
    ) -> Result<Option<LinkInfo>, web_dl_base::storable::Error> {
        macro_rules! target {
            ($($t:ident),+) => {
                match &self.target {
                    $(ActTarget::$t(t) => t.save_data(store, dest),)+
                    ActTarget::Other { .. } =>Ok(None)
                }
            };
        }
        return target!(Answer, Article, Collection, Column, Question, Pin);
    }
}

impl Item for Activity {
    type Reply = Reply;
    fn from_reply(reply: Self::Reply, raw_data: RawData) -> Self {
        macro_rules! target {
            ($($t:ident),+) => {
                match reply.target {
                    $(TargetReply::$t(t) => ActTarget::$t($t::from_reply(t, raw_data)),)+
                    TargetReply::Other => ActTarget::Other {
                        item: OtherItem::deserialize(&raw_data.data).ok(),
                        raw_data,
                    },
                }
            };
        }
        Activity {
            id: reply.id.0,
            target: target!(Answer, Article, Collection, Column, Question, Pin),
        }
    }
    async fn get_comments<P: crate::progress::ItemProg>(
        &mut self,
        client: &crate::request::Client,
        prog: &P,
    ) -> Result<(), crate::element::comment::FetchError> {
        macro_rules! target {
            ($($t:ident),+) => {
                match &mut self.target {
                    $(ActTarget::$t(t) => t.get_comments(client, prog).await,)+
                    ActTarget::Other { .. } => Ok(())
                }
            };
        }
        return target!(Answer, Article, Collection, Column, Pin, Question);
    }
    async fn get_images<P: crate::progress::ItemProg>(
        &mut self,
        client: &crate::request::Client,
        prog: &P,
    ) -> bool {
        macro_rules! target {
            ($($t:ident),+) => {
                match &mut self.target {
                    $(ActTarget::$t(t) => t.get_images(client, prog).await,)+
                    ActTarget::Other { .. } => false
                }
            };
        }
        return target!(Answer, Article, Collection, Column, Pin, Question);
    }
}

impl ItemContainer<Activity, VoidOpt> for super::User {
    async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
        client: &crate::request::Client,
        prog: &P,
        id: Self::Id<'a>,
        _: VoidOpt,
    ) -> Result<std::collections::LinkedList<RawData>, reqwest::Error> {
        client
            .get_paged_sign::<{ raw_data::Container::Activity }, Zse96V3, _, _>(
                prog.start_fetch(),
                format!("https://www.zhihu.com/api/v4/moments/{}/activities", id),
            )
            .await
    }
    async fn fixup<'a, P: crate::progress::ItemProg>(
        client: &crate::request::Client,
        prog: &P,
        _: Self::Id<'a>,
        _: VoidOpt,
        data: &mut Activity,
    ) -> Result<bool, reqwest::Error> {
        match &mut data.target {
            ActTarget::Article(a) => a.fix_cover(client, prog).await.map(|_| true),
            _ => Ok(false),
        }
    }
}
