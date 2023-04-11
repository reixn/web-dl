use crate::{
    element::{comment::HasComment, content::HasContent},
    item::{
        answer::{self, Answer},
        article::{self, Article},
        collection::{self, Collection},
        column::{self, Column},
        other::{OtherInfo, OtherItem},
        pin::{self, Pin},
        question::{self, Question},
        Item, ItemContainer, VoidOpt,
    },
    raw_data::{self, RawData, StrU64},
    request::Zse96V3,
    store::{self, LinkInfo, Store, StoreItem, StoreItemContainer},
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
impl HasComment for Activity {
    fn has_comment(&self) -> bool {
        macro_rules! target {
            ($($t:ident),+) => {
                match &self.target {
                    $(ActTarget::$t(t) => t.has_comment(),)+
                    ActTarget::Other {..} => false
                }
            };
        }
        target!(Answer, Article, Collection, Column, Pin, Question)
    }
    fn is_comment_fetched(&self) -> bool {
        macro_rules! target {
            ($($t:ident),+) => {
                match &self.target {
                    $(ActTarget::$t(t) => t.is_comment_fetched(),)+
                    ActTarget::Other {..} => true
                }
            };
        }
        target!(Answer, Article, Collection, Column, Pin, Question)
    }
    async fn get_comments<P: crate::progress::CommentTreeProg>(
        &mut self,
        prog: P,
        client: &crate::request::Client,
    ) -> Result<(), crate::element::comment::fetch::Error> {
        macro_rules! target {
                ($($t:ident),+) => {
                    match &mut self.target {
                        $(ActTarget::$t(t) => t.get_comments(prog, client).await,)+
                        ActTarget::Other { .. } => Ok(())
                    }
                };
            }
        target!(Answer, Article, Collection, Column, Pin, Question)
    }
}
impl StoreItem for Activity {
    fn in_store(id: Self::Id<'_>, store: &crate::store::Store) -> bool {
        macro_rules! target {
            ($($t:ident),+) => {
                match id.target {
                    $(ActTargetId::$t(t) => <$t as StoreItem>::in_store(t, store),)+
                    ActTargetId::Other(_) => false
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
        store: &mut Store,
    ) -> Result<Option<PathBuf>, web_dl_base::storable::Error> {
        macro_rules! target {
            ($($t:ident),+) => {
                match &self.target {
                    $(ActTarget::$t(t) => t.save_data(store),)+
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
        store: &mut crate::store::Store,
        dest: P,
    ) -> Result<Option<LinkInfo>, web_dl_base::storable::Error> {
        macro_rules! target {
            ($($t:ident),+) => {
                match &self.target {
                    $(ActTarget::$t(t) => t.save_data_link(store, dest),)+
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

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ActivityList {
    pub answer: BTreeSet<answer::AnswerId>,
    pub article: BTreeSet<article::ArticleId>,
    pub column: BTreeSet<column::ColumnId>,
    pub collection: BTreeSet<collection::CollectionId>,
    pub pin: BTreeSet<pin::PinId>,
    pub question: BTreeSet<question::QuestionId>,
}
impl StoreItemContainer<VoidOpt, Activity> for super::User {
    const OPTION_NAME: &'static str = "item";
    type ItemList = ActivityList;
    fn in_store(id: Self::Id<'_>, info: &store::ContainerInfo) -> bool {
        info.user.get(&id.0).map_or(false, |v| v.activity)
    }
    fn add_info(id: Self::Id<'_>, info: &mut store::ContainerInfo) {
        info.user.entry(id.0).or_default().activity = true;
    }
    fn add_item(id: <Activity as HasId>::Id<'_>, list: &mut Self::ItemList) {
        match id.target {
            ActTargetId::Answer(a) => {
                list.answer.insert(a);
            }
            ActTargetId::Article(a) => {
                list.article.insert(a);
            }
            ActTargetId::Collection(c) => {
                list.collection.insert(c);
            }
            ActTargetId::Column(c) => {
                list.column.insert(column::ColumnId(c.0.to_owned()));
            }
            ActTargetId::Pin(p) => {
                list.pin.insert(p);
            }
            ActTargetId::Question(q) => {
                list.question.insert(q);
            }
            ActTargetId::Other(_) => (),
        }
    }
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
