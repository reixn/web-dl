use std::fmt;

use super::types::*;
use anyhow::Context;
use clap::{Args, Subcommand};
use web_dl_base::{id::OwnedId, media};
use zhihu_dl::{
    driver::Driver,
    item::{
        any::Any,
        column::{self, Column},
        user::{self, User},
        Answer, Article, Collection, Item, ItemContainer, Pin, VoidOpt,
    },
    progress::progress_bar::ProgressReporter,
    store,
};

#[derive(Debug, Subcommand)]
pub enum ContainerOper<Id: Args> {
    Get {
        #[command(flatten)]
        id: Id,
        #[command(flatten)]
        get_opt: GetOpt,
    },
    Update {
        #[command(flatten)]
        id: Id,
        #[command(flatten)]
        get_opt: GetOpt,
    },
    Download {
        #[command(flatten)]
        id: Id,
        #[command(flatten)]
        get_opt: GetOpt,
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
        I: Item + media::HasImage + store::StoreItem,
        Id: OwnedId<IC>,
        IC: ItemContainer<O, I>,
    {
        if !driver.is_initialized() {
            anyhow::bail!("client is not initialized");
        }
        fn error_msg<I: Item, O, IC: ItemContainer<O, I>>(
            oper: &str,
            id: IC::Id<'_>,
            get_opt: GetOpt,
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
            Self::Get { id, get_opt } => {
                let id = id.to_id();
                driver
                    .get_container::<IC, I, O, _>(prog, id, get_opt.to_config())
                    .await
                    .with_context(|| error_msg::<I, O, IC>("get", id, get_opt, format_args!("")))?;
            }
            Self::Download {
                id,
                get_opt,
                link_opt,
            } => {
                let id = id.to_id();
                driver
                    .download_container::<IC, I, O, _, _>(
                        prog,
                        id,
                        get_opt.to_config(),
                        !link_opt.link_absolute,
                        &link_opt.dest,
                    )
                    .await
                    .with_context(|| {
                        error_msg::<I, O, IC>(
                            "download",
                            id,
                            get_opt,
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
            Self::Update { id, get_opt } => {
                let id = id.to_id();
                driver
                    .update_container::<IC, I, O, _>(prog, id, get_opt.to_config())
                    .await
                    .with_context(|| {
                        error_msg::<I, O, IC>("update", id, get_opt, format_args!(""))
                    })?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Subcommand)]
pub enum CollectionEntry {
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
    Collection {
        #[command(subcommand)]
        operation: CollectionEntry,
    },
    Column {
        #[command(subcommand)]
        operation: ColumnEntry,
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
            Self::Collection {
                operation: CollectionEntry::Item { operation },
            } => {
                operation
                    .run::<Collection, Any, VoidOpt>(driver, prog)
                    .await
            }
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
