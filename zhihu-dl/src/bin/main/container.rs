use super::types::*;
use anyhow::Context;
use clap::{Args, Subcommand};
use termcolor::Color;
use web_dl_base::{id::OwnedId, media};
use zhihu_dl::{
    driver::Driver,
    item::{
        any::Any,
        column::{self, Column},
        user::{self, User},
        Answer, Article, Collection, Item, ItemContainer, Pin, VoidOpt,
    },
    progress::{progress_bar::ProgressReporter, Reporter},
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
        output: &mut Output,
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
        let (start_tag, con_id, ok_tag, name, opt) = match &self {
            Self::Get { id, get_opt } => (
                "Getting",
                id.to_id().to_string(),
                "Got",
                "get",
                format!("({})", get_opt),
            ),
            Self::Download {
                id,
                get_opt,
                link_opt,
            } => (
                "Downloading",
                id.to_id().to_string(),
                "Downloaded",
                "download",
                format!(
                    "({}) to {}[{}]",
                    get_opt,
                    link_opt.dest,
                    if link_opt.link_absolute {
                        "link absolute"
                    } else {
                        "link relative"
                    }
                ),
            ),
            Self::Update { id, get_opt } => (
                "Updating",
                id.to_id().to_string(),
                "Updated",
                "update",
                format!("({})", get_opt),
            ),
        };
        output.write_tagged(
            Color::Cyan,
            start_tag,
            format_args_nl!(
                "{item} ({opt}) in {container} {con_id} {oper_opt}",
                item = I::TYPE,
                opt = IC::OPTION_NAME,
                container = IC::TYPE,
                con_id = con_id,
                oper_opt = opt
            ),
        );
        let v = match self {
            Self::Get { id, get_opt } => {
                let id = id.to_id();
                driver
                    .get_container::<IC, I, O, _>(
                        &prog.start_item_container(IC::TYPE, id, I::TYPE),
                        id,
                        get_opt.to_config(),
                    )
                    .await
            }
            Self::Download {
                id,
                get_opt,
                link_opt,
            } => {
                let id = id.to_id();
                driver
                    .download_container::<IC, I, O, _, _>(
                        &prog.start_item_container(IC::TYPE, id, I::TYPE),
                        id,
                        get_opt.to_config(),
                        !link_opt.link_absolute,
                        link_opt.dest,
                    )
                    .await
            }
            Self::Update { id, get_opt } => {
                let id = id.to_id();
                driver
                    .update_container::<IC, I, O, _>(
                        &prog.start_item_container(IC::TYPE, id, I::TYPE),
                        id,
                        get_opt.to_config(),
                    )
                    .await
                    .map(Option::Some)
            }
        }
        .with_context(|| {
            format!(
                "failed to {verb} {item} ({option}) in {container} {con_id} {oper_opt}",
                verb = name,
                item = I::TYPE,
                option = IC::OPTION_NAME,
                container = IC::TYPE,
                con_id = con_id,
                oper_opt = opt
            )
        })?;
        output.write_tagged(
            Color::Green,
            ok_tag,
            format_args_nl!(
                "{num}{item} ({opt}) in {container} {con_id} {oper_opt}",
                num = match v {
                    Some(v) => format!("{} ", v.len()),
                    None => String::new(),
                },
                item = I::TYPE,
                opt = IC::OPTION_NAME,
                container = IC::TYPE,
                con_id = con_id,
                oper_opt = opt
            ),
        );
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
        output: &mut Output,
        prog: &ProgressReporter,
    ) -> Result<(), anyhow::Error> {
        match self {
            Self::Collection {
                operation: CollectionEntry::Item { operation },
            } => {
                operation
                    .run::<Collection, Any, VoidOpt>(driver, output, prog)
                    .await
            }
            Self::Column { operation: op } => match op {
                ColumnEntry::Item { operation } => {
                    operation
                        .run::<Column, Any, column::Regular>(driver, output, prog)
                        .await
                }
                ColumnEntry::PinnedItem { operation } => {
                    operation
                        .run::<Column, Any, column::Pinned>(driver, output, prog)
                        .await
                }
            },
            Self::User { operation } => match operation {
                UserEntry::Answer { operation } => {
                    operation
                        .run::<User, Answer, VoidOpt>(driver, output, prog)
                        .await
                }
                UserEntry::Article { operation } => {
                    operation
                        .run::<User, Article, VoidOpt>(driver, output, prog)
                        .await
                }
                UserEntry::Collection { operation } => match operation {
                    UserCollection::Created { operation } => {
                        operation
                            .run::<User, Collection, user::Created>(driver, output, prog)
                            .await
                    }
                    UserCollection::Liked { operation } => {
                        operation
                            .run::<User, Collection, user::Liked>(driver, output, prog)
                            .await
                    }
                },
                UserEntry::Column { operation } => {
                    operation
                        .run::<User, Column, VoidOpt>(driver, output, prog)
                        .await
                }
                UserEntry::Pin { operation } => {
                    operation
                        .run::<User, Pin, VoidOpt>(driver, output, prog)
                        .await
                }
            },
        }
    }
}
