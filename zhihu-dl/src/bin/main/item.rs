use super::types::*;
use anyhow::Context;
use clap::{Args, Subcommand};
use std::{
    fmt::{self, Display},
    path::PathBuf,
};
use web_dl_base::{id::OwnedId, media, storable};
use zhihu_dl::{
    driver::Driver,
    element::content,
    item::{Answer, Article, Collection, Column, Fetchable, Item, Pin, Question, User},
    progress::{progress_bar::ProgressReporter, ItemJob, Reporter},
    store,
};

#[derive(Debug, Subcommand)]
pub enum ConvertOper {
    Pandoc {
        #[arg(long)]
        format: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ItemOper<Id: Args> {
    /// download and add to store, but not link
    Get {
        #[command(flatten)]
        id: Id,
        #[command(flatten)]
        get_opt: GetOpt,
    },
    AddRaw {
        #[command(flatten)]
        get_opt: GetOpt,
        #[arg(value_hint = clap::ValueHint::AnyPath)]
        path: String,
    },
    Download {
        #[command(flatten)]
        id: Id,
        #[command(flatten)]
        get_opt: GetOpt,
        #[command(flatten)]
        link_opt: LinkOpt,
    },
    Update {
        #[command(flatten)]
        id: Id,
        #[command(flatten)]
        get_opt: GetOpt,
    },
    ConvertHtml {
        #[command(flatten)]
        id: Id,
    },
    Convert {
        #[command(flatten)]
        id: Id,
        #[arg(long,value_hint=clap::ValueHint::AnyPath)]
        dest: String,
        #[command(subcommand)]
        operation: ConvertOper,
    },
}

fn error_msg<I: Item, Id: Display>(oper: &str, id: Id, opt: fmt::Arguments<'_>) -> String {
    format!(
        "failed to {oper} {kind} {id} {opt}",
        oper = oper,
        kind = I::TYPE,
        id = id,
        opt = opt
    )
}
async fn add_raw<I>(
    driver: &mut Driver,
    prog: &ProgressReporter,
    get_opt: GetOpt,
    path: &String,
) -> anyhow::Result<()>
where
    I: Item + media::HasImage + store::BasicStoreItem,
{
    let p = prog.start_item("Adding", "raw data of ", I::TYPE, path, get_opt);
    let v = driver
        .add_raw_item::<I, _>(
            &p,
            serde_json::from_reader(std::io::BufReader::new(
                std::fs::File::open(PathBuf::from(path.as_str()).as_path())
                    .with_context(|| format!("failed to open file {}", path))?,
            ))
            .context("failed to parse response to json value")?,
            get_opt.to_config(),
        )
        .await?;
    p.finish("Added", v.id());
    Ok(())
}
fn check_driver(driver: &Driver) -> Result<(), anyhow::Error> {
    if !driver.is_initialized() {
        anyhow::bail!("client is not initialized");
    }
    Ok(())
}
impl<Id: Args> ItemOper<Id> {
    async fn run<I>(self, driver: &mut Driver, prog: &ProgressReporter) -> Result<(), anyhow::Error>
    where
        I: Fetchable + Item + media::HasImage + store::BasicStoreItem,
        Id: OwnedId<I>,
    {
        match self {
            ItemOper::Get { id, get_opt } => {
                check_driver(driver)?;
                driver
                    .get_item::<I, _>(prog, id.to_id(), get_opt.to_config())
                    .await
                    .with_context(|| {
                        error_msg::<I, _>("get", id.to_id(), format_args!("({})", get_opt))
                    })?;
            }
            ItemOper::AddRaw { get_opt, path } => {
                check_driver(driver)?;
                add_raw::<I>(driver, prog, get_opt, &path)
                    .await
                    .with_context(|| {
                        error_msg::<I, _>("add raw data of", path, format_args!("{}", get_opt))
                    })?;
            }
            ItemOper::Download {
                id,
                get_opt,
                link_opt,
            } => {
                check_driver(driver)?;
                let id = id.to_id();
                driver
                    .download_item::<I, _, _>(
                        prog,
                        id,
                        get_opt.to_config(),
                        !link_opt.link_absolute,
                        PathBuf::from(link_opt.dest.as_str()),
                    )
                    .await
                    .with_context(|| {
                        error_msg::<I, _>(
                            "download",
                            id,
                            format_args!(
                                "({}) to {}[{}]",
                                get_opt,
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
            ItemOper::Update { id, get_opt } => {
                check_driver(driver)?;
                let id = id.to_id();
                driver
                    .update_item::<I, _>(prog, id, get_opt.to_config())
                    .await
                    .with_context(|| {
                        error_msg::<I, _>("update", id, format_args!("({})", get_opt))
                    })?;
            }
            ItemOper::ConvertHtml { id } => {
                let id = id.to_id();
                let p = prog.start_item("Converting", "raw html of ", I::TYPE, id, "");
                driver
                    .store
                    .get_object::<I>(id, storable::LoadOpt::default())
                    .context("failed to load object")
                    .and_then(|mut o| {
                        o.convert_html();
                        driver
                            .store
                            .add_object(&o)
                            .context("failed to store object")
                    })
                    .with_context(|| error_msg::<I, _>("convert raw html", id, format_args!("")))?;
                p.finish("Converted", id);
            }
            ItemOper::Convert {
                id,
                dest,
                operation: ConvertOper::Pandoc { format },
            } => {
                let id = id.to_id();
                let p = prog.start_item(
                    "Convert",
                    "document of ",
                    I::TYPE,
                    id,
                    format_args!("(using pandoc, {}) to {}", format, dest),
                );
                let v: Result<(), anyhow::Error> = try {
                    let obj = driver
                        .store
                        .get_object::<I>(id, storable::LoadOpt::default())
                        .context("failed to load object")?;
                    obj.get_main_content()
                        .context("can't find document")
                        .and_then(|d| d.document.as_ref().context("can't find document tree"))
                        .and_then(|d| {
                            use content::{
                                convertor::pandoc::{Pandoc, PandocConfig},
                                Convertor,
                            };
                            Pandoc::convert(
                                driver.store.image_path(),
                                d,
                                &PandocConfig {
                                    format: format.as_str(),
                                },
                                dest.as_str(),
                            )
                            .map_err(anyhow::Error::new)
                        })?;
                };
                v.with_context(|| {
                    error_msg::<I, _>(
                        "convert document of ",
                        id,
                        format_args!("(using pandoc, {}) to {}", format, dest),
                    )
                })?;
                p.finish("Converted", id);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Subcommand)]
pub enum ItemCmd {
    Answer {
        #[command(subcommand)]
        operation: ItemOper<NumId>,
    },
    Article {
        #[command(subcommand)]
        operation: ItemOper<NumId>,
    },
    Collection {
        #[command(subcommand)]
        operation: ItemOper<NumId>,
    },
    Column {
        #[command(subcommand)]
        operation: ItemOper<StrId>,
    },
    Pin {
        #[command(subcommand)]
        operation: ItemOper<NumId>,
    },
    Question {
        #[command(subcommand)]
        operation: ItemOper<NumId>,
    },
    User {
        #[command(subcommand)]
        operation: ItemOper<UserSpec>,
    },
}
impl ItemCmd {
    pub async fn run(
        self,
        driver: &mut Driver,
        prog: &ProgressReporter,
    ) -> Result<(), anyhow::Error> {
        macro_rules! run {
            ($($t:ident),*) => {
                match self {
                    $(ItemCmd::$t { operation } => {
                        operation
                            .run::<$t>(driver, prog)
                            .await
                    })+
                }
            };
        }
        run!(Answer, Article, Collection, Column, Question, Pin, User)
    }
}
