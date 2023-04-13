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

#[derive(Debug, HasImage, HasContent, Serialize, Deserialize)]
pub enum ActTarget {
    Answer(
        #[has_image]
        #[content(main)]
        Answer,
    ),
    Article(
        #[has_image]
        #[content(main)]
        Article,
    ),
    Collection(
        #[has_image]
        #[content(main)]
        Collection,
    ),
    Column(
        #[has_image]
        #[content(main)]
        Column,
    ),
    Pin(
        #[has_image]
        #[content(main)]
        Pin,
    ),
    Question(
        #[has_image]
        #[content(main)]
        Question,
    ),
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

#[derive(Debug, HasImage, HasContent, Serialize, Deserialize)]
pub struct Activity {
    pub id: u64,
    #[has_image(error = "pass_through")]
    #[content(main)]
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
                    ActTarget::Other(item) => ActivityId {id: self.id, target: ActTargetId::Other(item)}
                }
            };
        }
        target!(Answer, Article, Collection, Column, Pin, Question)
    }
}

impl StoreItem for Activity {
    fn in_store(id: Self::Id<'_>, store: &crate::store::Store) -> crate::store::info::ItemInfo {
        macro_rules! target {
            ($($t:ident),+) => {
                match id.target {
                    $(ActTargetId::$t(t) => <$t as StoreItem>::in_store(t, store),)+
                    ActTargetId::Other(_) => Default::default()
                }
            };
        }
        target!(Answer, Article, Collection, Column, Question, Pin)
    }
    fn add_info(id: Self::Id<'_>, info: crate::store::info::ItemInfo, store: &mut Store) {
        macro_rules! target {
            ($($t:ident),+) => {
                match id.target {
                    $(ActTargetId::$t(t) => <$t as StoreItem>::add_info(t, info, store),)+
                    ActTargetId::Other(_) => ()
                }
            };
        }
        target!(Answer, Article, Collection, Column, Question, Pin)
    }
    fn link_info<P: AsRef<Path>>(id: Self::Id<'_>, store: &Store, dest: P) -> Option<LinkInfo> {
        macro_rules! target {
            ($($t:ident),+) => {
                match id.target {
                    $(ActTargetId::$t(t) => $t::link_info(t, store, dest),)+
                    ActTargetId::Other(it) => {
                        it.warn();
                        None
                    }
                }
            };
        }
        target!(Answer, Article, Collection, Column, Question, Pin)
    }
    fn save_data(
        &self,
        on_server: bool,
        store: &mut Store,
    ) -> Result<Option<PathBuf>, web_dl_base::storable::Error> {
        macro_rules! target {
            ($($t:ident),+) => {
                match &self.target {
                    $(ActTarget::$t(t) => t.save_data(on_server, store),)+
                    ActTarget::Other(it) => {
                        it.warn();
                        Ok(None)
                    }
                }
            };
        }
        target!(Answer, Article, Collection, Column, Question, Pin)
    }
    fn save_data_link<P: AsRef<Path>>(
        &self,
        on_server: bool,
        store: &mut crate::store::Store,
        dest: P,
    ) -> Result<Option<LinkInfo>, web_dl_base::storable::Error> {
        macro_rules! target {
            ($($t:ident),+) => {
                match &self.target {
                    $(ActTarget::$t(t) => t.save_data_link(on_server, store, dest),)+
                    ActTarget::Other(it) => {
                        it.warn();
                        Ok(None)
                    }
                }
            };
        }
        target!(Answer, Article, Collection, Column, Question, Pin)
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
        macro_rules! target {
            ($($t:ident),+) => {
                match &mut self.target {
                    $(ActTarget::$t(t) => t.get_images(client, prog).await,)+
                    ActTarget::Other { .. } => false
                }
            };
        }
        target!(Answer, Article, Collection, Column, Pin, Question)
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
impl<'a, 'b> ContainerHandle<comment::Comment> for ActivityComCont<'a, 'b> {
    fn link_item(
        &mut self,
        id: <comment::Comment as HasId>::Id<'_>,
    ) -> Result<(), crate::store::StoreError> {
        match self {
            Self::Answer(a) => a.link_item(id),
            Self::Article(a) => a.link_item(id),
            Self::Collection(a) => a.link_item(id),
            Self::Pin(a) => a.link_item(id),
            Self::Question(a) => a.link_item(id),
            Self::Other => Ok(()),
        }
    }
    fn mark_missing(&mut self) {
        match self {
            Self::Answer(a) => a.mark_missing(),
            Self::Article(a) => a.mark_missing(),
            Self::Collection(c) => c.mark_missing(),
            Self::Pin(p) => p.mark_missing(),
            Self::Question(q) => q.mark_missing(),
            Self::Other => (),
        }
    }
    fn finish(self) -> Result<Option<PathBuf>, crate::store::StoreError> {
        match self {
            Self::Answer(a) => a.finish(),
            Self::Article(a) => a.finish(),
            Self::Collection(a) => a.finish(),
            Self::Pin(a) => a.finish(),
            Self::Question(a) => a.finish(),
            Self::Other => Ok(None),
        }
    }
}
impl StoreContainer<VoidOpt, comment::Comment> for Activity {
    const OPTION_NAME: &'static str = "comment";
    fn in_store(id: Self::Id<'_>, store: &Store) -> bool {
        macro_rules! target {
            ($($t:ident),+) => {
                match id.target {
                    $(ActTargetId::$t(t) => <$t as StoreContainer<VoidOpt, comment::Comment>>::in_store(t, store),)+
                    _ => true
                }
            };
        }
        target!(Answer, Article, Collection, Pin, Question)
    }
    fn store_path(id: Self::Id<'_>, store: &Store) -> Option<PathBuf> {
        macro_rules! target {
            ($($t:ident),+) => {
                match id.target {
                    $(ActTargetId::$t(t) => <$t as StoreContainer<VoidOpt, comment::Comment>>::store_path(t, store),)+
                    _ => None
                }
            };
        }
        target!(Answer, Article, Collection, Pin, Question)
    }
    type Handle<'a, 'b> = ActivityComCont<'a, 'b>;
    fn save_data<'a, 'b>(
        id: Self::Id<'a>,
        store: &'b mut Store,
    ) -> Result<Self::Handle<'a, 'b>, crate::store::StoreError> {
        macro_rules! target {
            ($($t:ident),+) => {
                match id.target {
                    $(ActTargetId::$t(t) => ActivityComCont::$t(<$t as StoreContainer<VoidOpt, comment::Comment>>::save_data(t, store)?),)+
                    _ => ActivityComCont::Other
                }
            };
        }
        Ok(target!(Answer, Article, Collection, Pin, Question))
    }
}
impl ItemContainer<VoidOpt, comment::Comment> for Activity {
    fn has_item(&self) -> bool {
        macro_rules! target {
            ($($t:ident),+) => {
                match &self.target {
                    $(ActTarget::$t(t) => <$t as ItemContainer<VoidOpt, comment::Comment>>::has_item(t),)+
                    _ => false
                }
            };
        }
        target!(Answer, Article, Collection, Pin, Question)
    }
    fn set_info(&self, has_item: bool) {
        macro_rules! target {
            ($($t:ident),+) => {
                match &self.target {
                    $(ActTarget::$t(t) => <$t as ItemContainer<VoidOpt, comment::Comment>>::set_info(t, has_item),)+
                    _ => ()
                }
            };
        }
        target!(Answer, Article, Collection, Pin, Question)
    }
    async fn fetch_items<'a, P: crate::progress::ItemContainerProg>(
        client: &crate::request::Client,
        prog: &P,
        id: Self::Id<'a>,
    ) -> Result<std::collections::LinkedList<RawData>, reqwest::Error> {
        macro_rules! target {
            ($($t:ident),+) => {
                match id.target {
                    $(ActTargetId::$t(t) => <$t as ItemContainer<VoidOpt,comment::Comment>>::fetch_items(client, prog, t).await,)+
                    _ => Ok(Default::default())
                }
            };
        }
        target!(Answer, Article, Collection, Pin, Question)
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
impl ItemList<Activity> for ActivityList {
    fn insert(&mut self, id: <Activity as HasId>::Id<'_>) {
        macro_rules! target {
            ($(($t:ident, $v:ident)),+) => {
                match id.target {
                    $(ActTargetId::$t(t) => ItemList::<$t>::insert(&mut self.$v, t),)+
                    ActTargetId::Other(_) => ()
                }
            };
        }
        target!(
            (Answer, answer),
            (Article, article),
            (Column, column),
            (Collection, collection),
            (Pin, pin),
            (Question, question)
        )
    }
    fn remove(&mut self, id: <Activity as HasId>::Id<'_>) {
        macro_rules! target {
            ($(($t:ident, $v:ident)),+) => {
                match id.target {
                    $(ActTargetId::$t(t) => ItemList::<$t>::remove(&mut self.$v, t),)+
                    ActTargetId::Other(_) => ()
                }
            };
        }
        target!(
            (Answer, answer),
            (Article, article),
            (Column, column),
            (Collection, collection),
            (Pin, pin),
            (Question, question)
        )
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
