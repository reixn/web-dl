use std::fmt;

use super::types::*;
use anyhow::Context;
use clap::{Args, Subcommand};
use web_dl_base::id::OwnedId;
use zhihu_dl::{
    driver::Driver,
    item::{
        any::Any,
        column::{self, Column},
        user::{self, User},
        Answer, Article, Collection, Comment, Item, ItemContainer, Pin, Question, VoidOpt,
    },
    progress::progress_bar::ProgressReporter,
    store,
};

#[derive(Debug, Subcommand)]
pub enum ContainerOper<Id: Args> {
    Get {
        #[command(flatten)]
        id: Id,
    },
    Update {
        #[command(flatten)]
        id: Id,
    },
    Download {
        #[command(flatten)]
        id: Id,
        #[command(flatten)]
        link_opt: LinkOpt,
    },
}
impl<Id: Args> ContainerOper<Id> {
    async fn run<IC, I, O>(
        self,
        driver: &mut Driver,
        prog: &ProgressReporter,
    ) -> Result<(), anyhow::Error>
    where
        I: Item + store::StoreItem,
        Id: OwnedId<IC>,
        IC: ItemContainer<O, I>,
    {
        if !driver.is_initialized() {
            anyhow::bail!("client is not initialized");
        }
        fn error_msg<I: Item, O, IC: ItemContainer<O, I>>(
            oper: &str,
            id: IC::Id<'_>,
            get_opt: fmt::Arguments<'_>,
            other: fmt::Arguments<'_>,
        ) -> String {
            format!(
                "failed to {op} {item} ({option}) in {container} {con_id} ({oper_opt}){other}",
                op = oper,
                item = I::TYPE,
                option = IC::OPTION_NAME,
                container = IC::TYPE,
                con_id = id,
                oper_opt = get_opt,
                other = other
            )
        }
        match self {
            Self::Get { id } => {
                let id = id.to_id();
                driver
                    .get_container::<IC, I, O, _>(prog, id)
                    .await
                    .with_context(|| {
                        error_msg::<I, O, IC>("get", id, format_args!(""), format_args!(""))
                    })?;
            }
            Self::Download { id, link_opt } => {
                let id = id.to_id();
                driver
                    .download_container::<IC, I, O, _, _>(
                        prog,
                        id,
                        !link_opt.link_absolute,
                        &link_opt.dest,
                    )
                    .await
                    .with_context(|| {
                        error_msg::<I, O, IC>(
                            "download",
                            id,
                            format_args!(""),
                            format_args!(
                                " to {}[{}]",
                                link_opt.dest,
                                if link_opt.link_absolute {
                                    "link absolute"
                                } else {
                                    "link relative"
                                }
                            ),
                        )
                    })?;
            }
            Self::Update { id } => {
                let id = id.to_id();
                driver
                    .update_container::<IC, I, O, _>(prog, id)
                    .await
                    .with_context(|| {
                        error_msg::<I, O, IC>("update", id, format_args!(""), format_args!(""))
                    })?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Subcommand)]
pub enum CommentEntry {
    Comment {
        #[command(subcommand)]
        operation: ContainerOper<NumId>,
    },
}
impl CommentEntry {
    async fn run<IC>(self, driver: &mut Driver, prog: &ProgressReporter) -> anyhow::Result<()>
    where
        IC: ItemContainer<VoidOpt, Comment>,
        NumId: OwnedId<IC>,
    {
        match self {
            Self::Comment { operation } => {
                operation.run::<IC, Comment, VoidOpt>(driver, prog).await
            }
        }
    }
}
#[derive(Debug, Subcommand)]
pub enum CollectionEntry {
    Comment {
        #[command(subcommand)]
        operation: ContainerOper<NumId>,
    },
    Item {
        #[command(subcommand)]
        operation: ContainerOper<NumId>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ColumnEntry {
    Item {
        #[command(subcommand)]
        operation: ContainerOper<StrId>,
    },
    PinnedItem {
        #[command(subcommand)]
        operation: ContainerOper<StrId>,
    },
}
#[derive(Debug, Subcommand)]
pub enum QuestionEntry {
    Answer {
        #[command(subcommand)]
        operation: ContainerOper<NumId>,
    },
    Comment {
        #[command(subcommand)]
        operation: ContainerOper<NumId>,
    },
}

#[derive(Debug, Subcommand)]
pub enum UserCollection {
    Created {
        #[command(subcommand)]
        operation: ContainerOper<UserSpec>,
    },
    Liked {
        #[command(subcommand)]
        operation: ContainerOper<UserSpec>,
    },
}
#[derive(Debug, Subcommand)]
pub enum UserEntry {
    Answer {
        #[command(subcommand)]
        operation: ContainerOper<UserSpec>,
    },
    Article {
        #[command(subcommand)]
        operation: ContainerOper<UserSpec>,
    },
    Column {
        #[command(subcommand)]
        operation: ContainerOper<UserSpec>,
    },
    Collection {
        #[command(subcommand)]
        operation: UserCollection,
    },
    Pin {
        #[command(subcommand)]
        operation: ContainerOper<UserSpec>,
    },
}
#[derive(Debug, Subcommand)]
pub enum ContainerCmd {
    Answer {
        #[command(subcommand)]
        operation: CommentEntry,
    },
    Article {
        #[command(subcommand)]
        operation: CommentEntry,
    },
    Collection {
        #[command(subcommand)]
        operation: CollectionEntry,
    },
    Comment {
        #[command(subcommand)]
        operation: CommentEntry,
    },
    Column {
        #[command(subcommand)]
        operation: ColumnEntry,
    },
    Pin {
        #[command(subcommand)]
        operation: CommentEntry,
    },
    Question {
        #[command(subcommand)]
        operation: QuestionEntry,
    },
    User {
        #[command(subcommand)]
        operation: UserEntry,
    },
}
impl ContainerCmd {
    pub async fn run(
        self,
        driver: &mut Driver,
        prog: &ProgressReporter,
    ) -> Result<(), anyhow::Error> {
        match self {
            Self::Answer { operation } => operation.run::<Answer>(driver, prog).await,
            Self::Article { operation } => operation.run::<Article>(driver, prog).await,
            Self::Collection { operation } => match operation {
                CollectionEntry::Comment { operation } => {
                    operation
                        .run::<Collection, Comment, VoidOpt>(driver, prog)
                        .await
                }
                CollectionEntry::Item { operation } => {
                    operation
                        .run::<Collection, Any, VoidOpt>(driver, prog)
                        .await
                }
            },
            Self::Column { operation: op } => match op {
                ColumnEntry::Item { operation } => {
                    operation
                        .run::<Column, Any, column::Regular>(driver, prog)
                        .await
                }
                ColumnEntry::PinnedItem { operation } => {
                    operation
                        .run::<Column, Any, column::Pinned>(driver, prog)
                        .await
                }
            },
            Self::Comment { operation } => operation.run::<Comment>(driver, prog).await,
            Self::Pin { operation } => operation.run::<Pin>(driver, prog).await,
            Self::Question { operation } => match operation {
                QuestionEntry::Answer { operation } => {
                    operation
                        .run::<Question, Comment, VoidOpt>(driver, prog)
                        .await
                }
                QuestionEntry::Comment { operation } => {
                    operation
                        .run::<Question, Comment, VoidOpt>(driver, prog)
                        .await
                }
            },
            Self::User { operation } => match operation {
                UserEntry::Answer { operation } => {
                    operation.run::<User, Answer, VoidOpt>(driver, prog).await
                }
                UserEntry::Article { operation } => {
                    operation.run::<User, Article, VoidOpt>(driver, prog).await
                }
                UserEntry::Collection { operation } => match operation {
                    UserCollection::Created { operation } => {
                        operation
                            .run::<User, Collection, user::Created>(driver, prog)
                            .await
                    }
                    UserCollection::Liked { operation } => {
                        operation
                            .run::<User, Collection, user::Liked>(driver, prog)
                            .await
                    }
                },
                UserEntry::Column { operation } => {
                    operation.run::<User, Column, VoidOpt>(driver, prog).await
                }
                UserEntry::Pin { operation } => {
                    operation.run::<User, Pin, VoidOpt>(driver, prog).await
                }
            },
        }
    }
}
