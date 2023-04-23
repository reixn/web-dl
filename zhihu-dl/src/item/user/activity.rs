use crate::{
    element::content::HasContent,
    item::{
        answer::{self, Answer},
        article::{self, Article},
        collection::{self, Collection},
        column::{self, Column},
        comment,
        other::{OtherInfo, OtherItem},
        pin::{self, Pin},
        question::{self, Question},
        Item, ItemContainer, VoidOpt,
    },
    raw_data::{self, RawData, StrU64},
    request::Zse96V3,
    store::{
        BasicStoreContainer, ContainerHandle, ItemList, LinkInfo, Store, StoreContainer, StoreItem,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fmt::Display,
    path::{Path, PathBuf},
};
use web_dl_base::id::HasId;

type Id<'a, S> = <S as HasId>::Id<'a>;

#[derive(Debug, Clone, Copy)]
pub enum ActTargetId<'a> {
    Answer(Id<'a, answer::Answer>),
    Article(Id<'a, article::Article>),
    Collection(Id<'a, collection::Collection>),
    Column(Id<'a, column::Column>),
    Pin(Id<'a, pin::Pin>),
    Question(Id<'a, question::Question>),
    Other(&'a OtherItem),
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
            Other(OtherItem { info: Some(i), .. }) => {
                f.write_fmt(format_args!("{} ({} {})", self.id, i.item_type, i.id))
            }
            Other(OtherItem { info: None, .. }) => {
                f.write_fmt(format_args!("{} (unknown)", self.id))
            }
        }
    }
}

#[derive(Debug, HasContent, Serialize, Deserialize)]
pub enum ActTarget {
    Answer(#[content(main)] Answer),
    Article(#[content(main)] Article),
    Collection(#[content(main)] Collection),
    Column(#[content(main)] Column),
    Pin(#[content(main)] Pin),
    Question(#[content(main)] Question),
    Other(OtherItem),
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

#[derive(Debug, HasContent, Serialize, Deserialize)]
pub struct Activity {
    pub id: u64,
    #[content(main)]
    pub target: ActTarget,
}

macro_rules! target {
    ($f:tt, $e:expr, $($t:ident),+) => {
        match $e {
            $(ActTarget::$t(t) => $f!($t, t),)+
            ActTarget::Other(_item) => $f!(_item)
        }
    };
}
macro_rules! targets {
    ($f:tt, $e:expr) => {
        target!($f, $e, Answer, Article, Collection, Column, Pin, Question)
    };
}

macro_rules! id_target {
    ($f:tt, $e:expr, $($t:ident),+) => {
        match $e {
            $(ActTargetId::$t(t) => $f!($t, t),)+
            ActTargetId::Other(_item) => $f!(_item)
        }
    };
}
macro_rules! id_targets {
    ($f:tt, $e:expr) => {
        id_target!($f, $e, Answer, Article, Collection, Column, Pin, Question)
    };
}

impl HasId for Activity {
    const TYPE: &'static str = "activity";
    type Id<'a> = ActivityId<'a>;
    fn id(&self) -> Self::Id<'_> {
        macro_rules! id_v {
            ($i:tt) => {
                ActTargetId::Other($i)
            };
            ($t:tt, $i:tt) => {
                ActTargetId::$t($i.id())
            };
        }
        ActivityId {
            id: self.id,
            target: targets!(id_v, &self.target),
        }
    }
}

impl StoreItem for Activity {
    fn in_store(id: Self::Id<'_>, store: &crate::store::Store) -> crate::store::info::ItemInfo {
        macro_rules! id_v {
            ($i:tt) => {
                Default::default()
            };
            ($t:tt, $i:tt) => {
                <$t as StoreItem>::in_store($i, store)
            };
        }
        id_targets!(id_v, id.target)
    }
    fn add_info(id: Self::Id<'_>, info: crate::store::info::ItemInfo, store: &mut Store) {
        macro_rules! id_v {
            ($i:tt) => {
                ()
            };
            ($t:tt, $i:tt) => {
                <$t as StoreItem>::add_info($i, info, store)
            };
        }
        id_targets!(id_v, id.target)
    }
    fn link_info<P: AsRef<Path>>(id: Self::Id<'_>, store: &Store, dest: P) -> Option<LinkInfo> {
        macro_rules! id_v {
            ($i:tt) => {{
                $i.warn();
                None
            }};
            ($t:tt, $i:tt) => {
                $t::link_info($i, store, dest)
            };
        }
        id_targets!(id_v, id.target)
    }
    fn add_media(&self, store: &mut Store) -> Result<(), web_dl_base::media::Error> {
        macro_rules! id_v {
            ($i:tt) => {
                Ok(())
            };
            ($t:tt, $i:tt) => {
                $i.add_media(store)
            };
        }
        targets!(id_v, &self.target)
    }
    fn save_data(
        &self,
        on_server: bool,
        store: &mut Store,
    ) -> Result<Option<PathBuf>, web_dl_base::storable::Error> {
        macro_rules! id_v {
            ($i:tt) => {{
                $i.warn();
                Ok(None)
            }};
            ($t:tt, $i:tt) => {
                $i.save_data(on_server, store)
            };
        }
        targets!(id_v, &self.target)
    }
    fn save_data_link<P: AsRef<Path>>(
        &self,
        on_server: bool,
        store: &mut crate::store::Store,
        dest: P,
    ) -> Result<Option<LinkInfo>, web_dl_base::storable::Error> {
        macro_rules! id_v {
            ($i:tt) => {{
                $i.warn();
                Ok(None)
            }};
            ($t:tt, $i:tt) => {
                $i.save_data_link(on_server, store, dest)
            };
        }
        targets!(id_v, &self.target)
    }
}

impl Item for Activity {
    type Reply = Reply;
    fn from_reply(reply: Self::Reply, raw_data: RawData) -> Self {
        macro_rules! target {
            ($($t:ident),+) => {
                match reply.target {
                    $(TargetReply::$t(t) => ActTarget::$t($t::from_reply(t, raw_data)),)+
                    TargetReply::Other => ActTarget::Other (OtherItem{
                        info: OtherInfo::deserialize(&raw_data.data).ok(),
                        raw_data,
                    }),
                }
            };
        }
        Activity {
            id: reply.id.0,
            target: target!(Answer, Article, Collection, Column, Question, Pin),
        }
    }
    async fn get_images<P: crate::progress::ItemProg>(
        &mut self,
        client: &crate::request::Client,
        prog: &P,
    ) -> bool {
        macro_rules! id_v {
            ($i:tt) => {
                false
            };
            ($t:tt, $i:tt) => {
                $i.get_images(client, prog).await
            };
        }
        targets!(id_v, &mut self.target)
    }
}

type CH<'a, 'b, T> = <T as StoreContainer<VoidOpt, comment::Comment>>::Handle<'a, 'b>;
pub enum ActivityComCont<'a, 'b> {
    Answer(CH<'a, 'b, Answer>),
    Article(CH<'a, 'b, Article>),
    Collection(CH<'a, 'b, Collection>),
    Pin(CH<'a, 'b, Pin>),
    Question(CH<'a, 'b, Question>),
    Other,
}
macro_rules! container_target {
    ($f:ident, $other:expr, $e:expr, $($t:ident),+) => {
        match $e {
            $(Self::$t(t) => $f!(t),)+
            Self::Other => $other
        }
    };
}
macro_rules! container_targets {
    ($f:ident, $other:expr, $e:expr) => {
        container_target!($f, $other, $e, Answer, Article, Collection, Pin, Question)
    };
}
impl<'a, 'b> ContainerHandle<comment::Comment> for ActivityComCont<'a, 'b> {
    fn link_item(
        &mut self,
        id: <comment::Comment as HasId>::Id<'_>,
    ) -> Result<(), crate::store::StoreError> {
        macro_rules! v {
            ($t:ident) => {
                $t.link_item(id)
            };
        }
        container_targets!(v, Ok(()), self)
    }
    fn mark_missing(&mut self) {
        macro_rules! v {
            ($t:ident) => {
                $t.mark_missing()
            };
        }
        container_targets!(v, (), self)
    }
    fn finish(self) -> Result<Option<PathBuf>, crate::store::StoreError> {
        macro_rules! v {
            ($t:ident) => {
                $t.finish()
            };
        }
        container_targets!(v, Ok(None), self)
    }
}

macro_rules! call_fun {
    ($t:ident, $v:ident, $tr:tt, $f:ident, ($($a:tt),*)) => {
        <$t as $tr<VoidOpt, comment::Comment>>::$f($v, $($a,)*)
    };
}
macro_rules! id_target {
    ($other:expr, $e:expr, $tr:tt, $f:ident, $a:tt, $($t:ident),+) => {
        match $e {
            $(ActTargetId::$t(t) => call_fun!($t, t, $tr, $f, $a),)+
            _ => $other
        }
    };
}
macro_rules! id_targets {
    ($other:expr, $e:expr, $tr:tt, $f:ident, $a:tt) => {
        id_target!($other, $e, $tr, $f, $a, Answer, Article, Collection, Pin, Question)
    };
}

impl StoreContainer<VoidOpt, comment::Comment> for Activity {
    const OPTION_NAME: &'static str = "comment";
    fn in_store(id: Self::Id<'_>, store: &Store) -> bool {
        id_targets!(true, id.target, StoreContainer, in_store, (store))
    }
    fn store_path(id: Self::Id<'_>, store: &Store) -> Option<PathBuf> {
        id_targets!(None, id.target, StoreContainer, store_path, (store))
    }
    type Handle<'a, 'b> = ActivityComCont<'a, 'b>;
    fn save_data<'a, 'b>(
        id: Self::Id<'a>,
        store: &'b mut Store,
    ) -> Result<Self::Handle<'a, 'b>, crate::store::StoreError> {
        macro_rules! call_fun {
            ($t:ident, $v:ident, $tr:tt, $f:ident, $a:tt) => {
                ActivityComCont::$t(<$t as $tr<VoidOpt, comment::Comment>>::$f($v, store)?)
            };
        }
        Ok(id_targets!(
            ActivityComCont::Other,
            id.target,
            StoreContainer,
            save_data,
            ()
        ))
    }
}

macro_rules! target {
    ($other:expr, $e:expr, $tr:ident, $f:ident, $a:tt, $($t:ident),+) => {
        match $e {
            $(ActTarget::$t(t) => call_fun!($t, t, $tr, $f, $a),)+
            _ => $other
        }
    };
}
macro_rules! targets {
    ($other:expr, $e:expr, $t:ident, $f:ident, $a:tt) => {
        target!($other, $e, $t, $f, $a, Answer, Article, Collection, Pin, Question)
    };
}
impl ItemContainer<VoidOpt, comment::Comment> for Activity {
    fn has_item(&self) -> bool {
        targets!(false, &self.target, ItemContainer, has_item, ())
    }
    fn set_info(&self, has_item: bool) {
        targets!((), &self.target, ItemContainer, set_info, (has_item))
    }
    async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
        client: &crate::request::Client,
        prog: &P,
        id: Self::Id<'a>,
    ) -> Result<std::collections::LinkedList<RawData>, reqwest::Error> {
        macro_rules! call_fun {
            ($t:ident, $v:ident, $tr:tt, $f:ident, $a:tt) => {
                <$t as $tr<VoidOpt, comment::Comment>>::$f(client, prog, $v).await
            };
        }
        id_targets!(
            Ok(Default::default()),
            id.target,
            ItemContainer,
            fetch_items,
            ()
        )
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActivityList {
    pub answer: BTreeSet<answer::AnswerId>,
    pub article: BTreeSet<article::ArticleId>,
    pub column: BTreeSet<column::ColumnId>,
    pub collection: BTreeSet<collection::CollectionId>,
    pub pin: BTreeSet<pin::PinId>,
    pub question: BTreeSet<question::QuestionId>,
}
macro_rules! list_target {
    ($e:expr, $s:expr, $f:ident, $(($t:ident, $v:ident)),+) => {
        match $e {
            $(ActTargetId::$t(t) => ItemList::<$t>::$f(&mut $s.$v, t),)+
            ActTargetId::Other(_) => ()
        }
    };
}
macro_rules! list_targets {
    ($e:expr, $s:expr, $f:ident) => {
        list_target!(
            $e,
            $s,
            $f,
            (Answer, answer),
            (Article, article),
            (Column, column),
            (Collection, collection),
            (Pin, pin),
            (Question, question)
        )
    };
}
impl ItemList<Activity> for ActivityList {
    fn insert(&mut self, id: <Activity as HasId>::Id<'_>) {
        list_targets!(id.target, self, insert)
    }
    fn remove(&mut self, id: <Activity as HasId>::Id<'_>) {
        list_targets!(id.target, self, remove)
    }
    fn set_item_info(&self, info: crate::store::info::ItemInfo, store: &mut Store) {
        macro_rules! target {
            ($($t:ident),+) => {
                $(self.$t.set_item_info(info, store);)+
            };
        }
        target!(answer, article, column, collection, pin, question);
    }
}
impl BasicStoreContainer<VoidOpt, Activity> for super::User {
    const OPTION_NAME: &'static str = "item";
    type ItemList = ActivityList;
    container_info!(activity);
}
impl ItemContainer<VoidOpt, Activity> for super::User {
    async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
        client: &crate::request::Client,
        prog: &P,
        id: Self::Id<'a>,
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
        data: &mut Activity,
    ) -> Result<bool, reqwest::Error> {
        match &mut data.target {
            ActTarget::Article(a) => a.fix_cover(client, prog).await.map(|_| true),
            _ => Ok(false),
        }
    }
}
