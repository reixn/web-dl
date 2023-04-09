use crate::{
    element::author::UserId,
    item::{
        self,
        answer::{Answer, AnswerId},
        any::Any,
        article::{Article, ArticleId},
        collection::CollectionId,
        column::{ColumnId, ColumnRef},
        pin::PinId,
        question::QuestionId,
        Collection, Column, Fetchable, Item, ItemContainer, Pin, Question, User, VoidOpt,
    },
    progress::Reporter,
    store::{BasicStoreItem, StoreError, StoreItemContainer},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    fs, io,
    path::PathBuf,
};
use web_dl_base::id::HasId;

mod option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub fn serialize<T: Serialize, S: Serializer>(
        value: &Option<T>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(v) => v.serialize(serializer),
            None => ().serialize(serializer),
        }
    }
    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<T>, D::Error> {
        T::deserialize(deserializer).map(Option::Some)
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ItemOption {
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub get: Option<super::GetConfig>,
}

pub type AnswerOption = ItemOption;

pub type ArticleOption = ItemOption;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct CollectionOption {
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub container: Option<ItemOption>,
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub item: Option<ItemOption>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ColumnOption {
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub container: Option<ItemOption>,
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub pinned: Option<ItemOption>,
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub regular: Option<ItemOption>,
}

pub type PinOption = ItemOption;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct QuestionOption {
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub container: Option<ItemOption>,
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub answer: Option<ItemOption>,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct UserCollection {
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub created: Option<CollectionOption>,
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub liked: Option<CollectionOption>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserOption {
    pub id: UserId,
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub container: Option<ItemOption>,
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub answer: Option<AnswerOption>,
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub article: Option<AnswerOption>,
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub collection: Option<UserCollection>,
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub column: Option<ColumnOption>,
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub pin: Option<PinOption>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Manifest {
    Leaf {
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        answer: BTreeMap<AnswerId, AnswerOption>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        article: BTreeMap<ArticleId, ArticleOption>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        collection: BTreeMap<CollectionId, CollectionOption>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        column: BTreeMap<ColumnId, ColumnOption>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        pin: BTreeMap<PinId, PinOption>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        question: BTreeMap<QuestionId, QuestionOption>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        user: BTreeMap<String, UserOption>,
    },
    Branch(BTreeMap<String, Manifest>),
}

#[derive(Debug)]
pub enum FsErrorOp {
    CreateDir,
    SymLink(PathBuf),
}
impl Display for FsErrorOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateDir => f.write_str("create directory"),
            Self::SymLink(p) => write!(f, "synlink to {} from", p.display()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to get current working directory")]
    GetCwd(#[source] io::Error),
    #[error("failed to {op} {}", path.display())]
    Fs {
        op: FsErrorOp,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to process item {kind} {id}")]
    Item {
        id: String,
        kind: &'static str,
        #[source]
        source: super::ItemError,
    },
    #[error("failed to process {item_kind} ({option}) in {kind} {id}")]
    Container {
        item_kind: &'static str,
        id: String,
        kind: &'static str,
        option: &'static str,
        #[source]
        source: super::ContainerError,
    },
    #[error("failed to list {kind} ({option}) of user {id}")]
    ListContainer {
        id: String,
        kind: &'static str,
        option: &'static str,
        #[source]
        source: StoreError,
    },
    #[error("failed to process {kind} ({option}) of user {id}")]
    SubContainer {
        id: String,
        kind: &'static str,
        option: &'static str,
        #[source]
        source: Box<Self>,
    },
}

fn create_dir(path: &PathBuf) -> Result<(), Error> {
    fs::create_dir(&path).map_err(|e| Error::Fs {
        op: FsErrorOp::CreateDir,
        path: path.clone(),
        source: e,
    })
}

impl super::Driver {
    async fn apply_item<I: Fetchable + Item + BasicStoreItem, P: Reporter>(
        &mut self,
        prog: &P,
        id: I::Id<'_>,
        config: super::GetConfig,
        path: PathBuf,
    ) -> Result<(), Error> {
        if !path.exists() {
            self.download_item::<I, _, _>(prog, id, config, true, &path)
                .await
                .map_err(|e| Error::Item {
                    id: id.to_string(),
                    kind: I::TYPE,
                    source: e,
                })?;
        }
        Ok(())
    }
    async fn apply_container<IC: ItemContainer<O, I>, O, I: Item, P: Reporter>(
        &mut self,
        prog: &P,
        id: IC::Id<'_>,
        config: super::GetConfig,
        path: &PathBuf,
    ) -> Result<(), Error> {
        let path = path.join(IC::OPTION_NAME);
        if !path.exists() {
            self.download_container::<IC, I, O, P, _>(prog, id, config, true, &path)
                .await
                .map_err(|e| Error::Container {
                    item_kind: I::TYPE,
                    id: id.to_string(),
                    kind: IC::TYPE,
                    option: IC::OPTION_NAME,
                    source: e,
                })?;
        }
        Ok(())
    }
    fn list_container<IC, O, I>(&mut self, id: IC::Id<'_>) -> Result<IC::ItemList, Error>
    where
        I: Item,
        IC: ItemContainer<O, I>,
    {
        self.store
            .get_container::<O, I, IC>(id)
            .map_err(|e| Error::ListContainer {
                id: id.to_string(),
                kind: I::TYPE,
                option: IC::OPTION_NAME,
                source: e,
            })
    }
    async fn get_sub_container<'a, IC, O, I, SI, SO, P: Reporter, It>(
        &mut self,
        prog: &P,
        config: super::GetConfig,
        id: IC::Id<'a>,
        sub_ids: It,
    ) -> Result<(), Error>
    where
        SI: Item,
        I: Item + ItemContainer<SO, SI> + 'a,
        IC: ItemContainer<O, I>,
        It: Iterator<Item = &'a I::Id<'a>>,
    {
        for i in sub_ids {
            self.get_container::<I, SI, SO, _>(prog, *i, config)
                .await
                .map_err(|e| Error::SubContainer {
                    id: id.to_string(),
                    kind: IC::TYPE,
                    option: IC::OPTION_NAME,
                    source: Box::new(Error::Container {
                        item_kind: SI::TYPE,
                        id: i.to_string(),
                        kind: I::TYPE,
                        option: I::OPTION_NAME,
                        source: e,
                    }),
                })?;
        }
        Ok(())
    }
    async fn apply_user_collection<O, P: Reporter>(
        &mut self,
        prog: &P,
        id: item::user::StoreId<'_>,
        config: CollectionOption,
        path: &PathBuf,
    ) -> Result<(), Error>
    where
        User: ItemContainer<O, Collection>
            + StoreItemContainer<O, Collection, ItemList = BTreeSet<CollectionId>>,
    {
        self.apply_container::<User, O, Collection, _>(
            prog,
            id,
            config.container.unwrap_or_default().get.unwrap_or_default(),
            &path,
        )
        .await?;
        if let Some(c) = config.item {
            let list = self.list_container::<User, O, Collection>(id)?;
            self.get_sub_container::<User, O, Collection, Any, VoidOpt, _, _>(
                prog,
                c.get.unwrap_or_default(),
                id,
                list.iter(),
            )
            .await?;
        }
        Ok(())
    }
    #[async_recursion::async_recursion(?Send)]
    async fn apply_manifest_impl<P: Reporter>(
        &mut self,
        prog: &P,
        manifest: &Manifest,
        path: PathBuf,
    ) -> Result<(), Error> {
        match manifest {
            Manifest::Leaf {
                answer,
                article,
                collection,
                column,
                pin,
                question,
                user,
            } => {
                if !answer.is_empty() {
                    let path = path.join(Answer::TYPE);
                    for (id, opt) in answer {
                        self.apply_item::<Answer, _>(
                            prog,
                            *id,
                            opt.get.unwrap_or_default(),
                            path.join(id.to_string()),
                        )
                        .await?;
                    }
                }
                if !article.is_empty() {
                    let path = path.join(Article::TYPE);
                    for (id, opt) in article {
                        self.apply_item::<Article, _>(
                            prog,
                            *id,
                            opt.get.unwrap_or_default(),
                            path.join(id.to_string()),
                        )
                        .await?;
                    }
                }
                if !collection.is_empty() {
                    let path = path.join(Collection::TYPE);
                    for (id, opt) in collection {
                        let path = path.join(id.to_string());
                        if let Some(c) = opt.container {
                            self.apply_item::<Collection, _>(
                                prog,
                                *id,
                                c.get.unwrap_or_default(),
                                path.join("info"),
                            )
                            .await?;
                        }
                        if let Some(v) = opt.item {
                            self.apply_container::<Collection, VoidOpt, Any, _>(
                                prog,
                                *id,
                                v.get.unwrap_or_default(),
                                &path,
                            )
                            .await?;
                        }
                    }
                }
                if !column.is_empty() {
                    let path = path.join(Column::TYPE);
                    for (id, opt) in column {
                        let path = path.join(&id.0);
                        let id = ColumnRef(&id.0);
                        if let Some(c) = opt.container {
                            self.apply_item::<Column, _>(
                                prog,
                                id,
                                c.get.unwrap_or_default(),
                                path.join("info"),
                            )
                            .await?;
                        }
                        if let Some(c) = opt.pinned {
                            self.apply_container::<Column, item::column::Pinned, Any, _>(
                                prog,
                                id,
                                c.get.unwrap_or_default(),
                                &path,
                            )
                            .await?;
                        }
                        if let Some(c) = opt.regular {
                            self.apply_container::<Column, item::column::Regular, Any, _>(
                                prog,
                                id,
                                c.get.unwrap_or_default(),
                                &path,
                            )
                            .await?;
                        }
                    }
                }
                if !pin.is_empty() {
                    let path = path.join(Pin::TYPE);
                    for (id, opt) in pin {
                        self.apply_item::<Pin, _>(
                            prog,
                            *id,
                            opt.get.unwrap_or_default(),
                            path.join(id.to_string()),
                        )
                        .await?;
                    }
                }
                if !question.is_empty() {
                    let path = path.join(Question::TYPE);
                    for (id, opt) in question {
                        let path = path.join(id.to_string());
                        if let Some(c) = opt.container {
                            self.apply_item::<Question, _>(
                                prog,
                                *id,
                                c.get.unwrap_or_default(),
                                path.join("info"),
                            )
                            .await?;
                        }
                        if let Some(c) = opt.answer {
                            self.apply_container::<Question, VoidOpt, Answer, _>(
                                prog,
                                *id,
                                c.get.unwrap_or_default(),
                                &path,
                            )
                            .await?;
                        }
                    }
                }
                if !user.is_empty() {
                    let path = path.join(User::TYPE);
                    for (id, opt) in user {
                        let path = path.join(id);
                        let id = item::user::StoreId(opt.id, id.as_str());
                        if let Some(c) = opt.container {
                            self.apply_item::<User, _>(
                                prog,
                                id,
                                c.get.unwrap_or_default(),
                                path.join("info"),
                            )
                            .await?;
                        }
                        if let Some(c) = opt.answer {
                            self.apply_container::<User, VoidOpt, Answer, _>(
                                prog,
                                id,
                                c.get.unwrap_or_default(),
                                &path,
                            )
                            .await?;
                        }
                        if let Some(c) = opt.article {
                            self.apply_container::<User, VoidOpt, Article, _>(
                                prog,
                                id,
                                c.get.unwrap_or_default(),
                                &path,
                            )
                            .await?;
                        }
                        if let Some(c) = opt.collection {
                            if let Some(c) = c.created {
                                self.apply_user_collection::<item::user::Created, _>(
                                    prog, id, c, &path,
                                )
                                .await?;
                            }
                            if let Some(c) = c.liked {
                                self.apply_user_collection::<item::user::Liked, _>(
                                    prog, id, c, &path,
                                )
                                .await?;
                            }
                        }
                        if let Some(c) = opt.column {
                            self.apply_container::<User, VoidOpt, Column, _>(
                                prog,
                                id,
                                c.container.unwrap_or_default().get.unwrap_or_default(),
                                &path,
                            )
                            .await?;
                            let list = self.list_container::<User, VoidOpt, Column>(id)?;
                            let list: Vec<ColumnRef> =
                                list.iter().map(|v| ColumnRef(v.0.as_str())).collect();
                            if let Some(c) = c.pinned {
                                self.get_sub_container::<User, VoidOpt, Column, Any, item::column::Pinned, _, _>
                                    (prog, c.get.unwrap_or_default(), id, list.iter()).await?;
                            }
                            if let Some(c) = c.regular {
                                self.get_sub_container::<User, VoidOpt, Column, Any, item::column::Regular, _, _>
                                    (prog, c.get.unwrap_or_default(), id, list.iter()).await?;
                            }
                        }
                        if let Some(c) = opt.pin {
                            self.apply_container::<User, VoidOpt, Pin, _>(
                                prog,
                                id,
                                c.get.unwrap_or_default(),
                                &path,
                            )
                            .await?;
                        }
                    }
                }
            }
            Manifest::Branch(b) => {
                for (name, m) in b {
                    let path = path.join(name);
                    if !path.exists() {
                        create_dir(&path)?;
                    }
                    self.apply_manifest_impl(prog, m, path).await?;
                }
            }
        }
        Ok(())
    }
    pub async fn apply_manifest<P: Reporter>(
        &mut self,
        prog: &P,
        manifest: &Manifest,
    ) -> Result<(), Error> {
        self.apply_manifest_impl(
            prog,
            manifest,
            std::env::current_dir().map_err(Error::GetCwd)?,
        )
        .await
    }
}
