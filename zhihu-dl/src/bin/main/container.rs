use super::types::*;
use anyhow::Context;
use clap::{Args, Subcommand, ValueEnum};
use std::fmt::Display;
use termcolor::Color;
use web_dl_base::{id::OwnedId, media};
use zhihu_dl::{
    driver::Driver,
    item::{
        any::Any,
        column::{Column, ColumnItem},
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
        option: O,
    ) -> Result<(), anyhow::Error>
    where
        I: Item + media::HasImage + store::StoreItem,
        Id: OwnedId<IC>,
        O: Display + Copy,
        IC: ItemContainer<I, O>,
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
        };
        output.write_tagged(
            Color::Cyan,
            start_tag,
            format_args_nl!(
                "{item} ({opt}) in {container} {con_id} {oper_opt}",
                item = I::TYPE,
                opt = option,
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
                        option,
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
                        option,
                        get_opt.to_config(),
                        !link_opt.link_absolute,
                        link_opt.dest,
                    )
                    .await
            }
        }
        .with_context(|| {
            format!(
                "failed to {verb} {item} ({option}) in {container} {con_id} {oper_opt}",
                verb = name,
                item = I::TYPE,
                option = option,
                container = IC::TYPE,
                con_id = con_id,
                oper_opt = opt
            )
        })?;
        output.write_tagged(
            Color::Green,
            ok_tag,
            format_args_nl!(
                "{num} {item} ({opt}) in {container} {con_id} {oper_opt}",
                num = v.len(),
                item = I::TYPE,
                opt = option,
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
        #[arg(long)]
        pinned: bool,
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

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CollectionTyp {
    Liked,
    Created,
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
        #[arg(long = "type")]
        typ: CollectionTyp,
        #[command(subcommand)]
        operation: ContainerOper<UserSpec>,
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
                    .run::<Collection, Any, _>(driver, output, prog, VoidOpt)
                    .await
            }
            Self::Column {
                operation: ColumnEntry::Item { pinned, operation },
            } => {
                operation
                    .run::<Column, Any, _>(
                        driver,
                        output,
                        prog,
                        if pinned {
                            ColumnItem::Pinned
                        } else {
                            ColumnItem::Regular
                        },
                    )
                    .await
            }
            Self::User { operation } => match operation {
                UserEntry::Answer { operation } => {
                    operation
                        .run::<User, Answer, _>(driver, output, prog, VoidOpt)
                        .await
                }
                UserEntry::Article { operation } => {
                    operation
                        .run::<User, Article, _>(driver, output, prog, VoidOpt)
                        .await
                }
                UserEntry::Collection { typ, operation } => {
                    operation
                        .run::<User, Collection, _>(
                            driver,
                            output,
                            prog,
                            match typ {
                                CollectionTyp::Created => user::CollectionOpt::Created,
                                CollectionTyp::Liked => user::CollectionOpt::Liked,
                            },
                        )
                        .await
                }
                UserEntry::Column { operation } => {
                    operation
                        .run::<User, Column, _>(driver, output, prog, VoidOpt)
                        .await
                }
                UserEntry::Pin { operation } => {
                    operation
                        .run::<User, Pin, _>(driver, output, prog, VoidOpt)
                        .await
                }
            },
        }
    }
}
