use super::types::*;
use anyhow::Context;
use clap::{Args, Subcommand};
use rustyline::hint::Hint;
use std::path::PathBuf;
use termcolor::Color;
use web_dl_base::{id::OwnedId, media, storable};
use zhihu_dl::{
    driver::Driver,
    element::content,
    item::{Answer, Article, Collection, Column, Fetchable, Item, Pin, Question, User},
    progress::{progress_bar::ProgressReporter, Reporter},
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

async fn add_raw<I>(
    driver: &mut Driver,
    prog: &ProgressReporter,
    get_opt: GetOpt,
    path: String,
) -> anyhow::Result<String>
where
    I: Item + media::HasImage + store::BasicStoreItem,
{
    match driver
        .add_raw_item::<I, _>(
            &prog.start_item(I::TYPE, "<raw data>"),
            serde_json::from_reader(std::io::BufReader::new(
                std::fs::File::open(PathBuf::from(path.as_str()).as_path())
                    .with_context(|| format!("failed to open file {}", path))?,
            ))
            .context("failed to parse response to json value")?,
            get_opt.to_config(),
        )
        .await
    {
        Ok(i) => Ok(i.id().to_string()),
        Err(e) => Err(anyhow::Error::new(e)),
    }
}
impl<Id: Args> ItemOper<Id> {
    async fn run<I>(
        self,
        driver: &mut Driver,
        output: &mut Output,
        prog: &ProgressReporter,
    ) -> Result<(), anyhow::Error>
    where
        I: Fetchable + Item + media::HasImage + store::BasicStoreItem,
        Id: OwnedId<I>,
    {
        let (start_tag, pre, start_id, ok_tag, name, opt, require_init) = match &self {
            Self::Get { id, get_opt } => (
                "Getting",
                "",
                format!("{}", id.to_id()),
                "Got",
                "get",
                format!("({})", get_opt),
                true,
            ),
            Self::AddRaw { get_opt, path } => (
                "Adding",
                "raw data of ",
                String::from("<raw data>"),
                "Added",
                "add raw data of",
                format!("({}) from {}", get_opt, path),
                true,
            ),
            Self::Download {
                get_opt,
                id,
                link_opt,
            } => (
                "Downloading",
                "",
                format!(" {}", id.to_id()),
                "Downloaded",
                "download",
                format!(
                    "({}) to {}[{}]",
                    get_opt,
                    link_opt.dest.display(),
                    if link_opt.link_absolute {
                        "link absolute"
                    } else {
                        "link relative"
                    }
                ),
                true,
            ),
            Self::Update { id, get_opt } => (
                "Updating",
                "",
                format!(" {}", id.to_id()),
                "Updated",
                "update",
                format!("({})", get_opt),
                true,
            ),
            Self::ConvertHtml { id } => (
                "Converting",
                "raw html of ",
                format!(" {}", id.to_id()),
                "Converted",
                "convert raw html of",
                String::new(),
                false,
            ),
            Self::Convert {
                id,
                dest,
                operation: ConvertOper::Pandoc { format },
            } => (
                "Converting",
                "document of ",
                format!(" {}", id.to_id()),
                "Converted",
                "convert document of",
                format!("using pandoc to {} {}", format, dest),
                false,
            ),
        };
        if require_init && !driver.is_initialized() {
            anyhow::bail!("client is not initialized");
        }
        output.write_tagged(
            Color::Cyan,
            start_tag,
            format_args_nl!(
                "{pre}{item} {id} {opt}",
                pre = pre,
                item = I::TYPE,
                id = start_id,
                opt = opt
            ),
        );
        let id = match self {
            ItemOper::Get { id, get_opt } => {
                let id = id.to_id();
                match driver
                    .get_item::<I, _>(&prog.start_item(I::TYPE, id), id, get_opt.to_config())
                    .await
                {
                    Ok(_) => Ok(id.to_string()),
                    Err(e) => Err(anyhow::Error::new(e)),
                }
            }
            ItemOper::AddRaw { get_opt, path } => add_raw::<I>(driver, prog, get_opt, path).await,
            ItemOper::Download {
                id,
                get_opt,
                link_opt,
            } => {
                let id = id.to_id();
                match driver
                    .download_item::<I, _, _>(
                        &prog.start_item(I::TYPE, id),
                        id,
                        get_opt.to_config(),
                        !link_opt.link_absolute,
                        PathBuf::from(link_opt.dest.as_str()),
                    )
                    .await
                {
                    Ok(_) => Ok(id.to_string()),
                    Err(e) => Err(anyhow::Error::new(e)),
                }
            }
            ItemOper::Update { id, get_opt } => {
                let id = id.to_id();
                match driver
                    .update_item::<I, _>(&prog.start_item(I::TYPE, id), id, get_opt.to_config())
                    .await
                {
                    Ok(_) => Ok(id.to_string()),
                    Err(e) => Err(anyhow::Error::new(e)),
                }
            }
            ItemOper::ConvertHtml { id } => {
                let id = id.to_id();
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
                    .map(|_| id.to_string())
            }
            ItemOper::Convert {
                id,
                dest,
                operation,
            } => {
                try {
                    let id = id.to_id();
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
                                    format: match operation {
                                        ConvertOper::Pandoc { format } => format,
                                    },
                                },
                                dest.as_str(),
                            )
                            .map_err(anyhow::Error::new)
                        })
                        .map(|_| id.to_string())?
                }
            }
        }
        .with_context(|| {
            format!(
                "failed to {verb} {item} {id} {opt}",
                verb = name,
                item = I::TYPE,
                id = start_id,
                opt = opt
            )
        })?;
        output.write_tagged(
            Color::Green,
            ok_tag,
            format_args_nl!(
                "{pre}{item} {id} {opt}",
                pre = pre,
                item = I::TYPE,
                id = id,
                opt = opt
            ),
        );
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
        output: &mut Output,
        prog: &ProgressReporter,
    ) -> Result<(), anyhow::Error> {
        macro_rules! run {
            ($($t:ident),*) => {
                match self {
                    $(ItemCmd::$t { operation } => {
                        operation
                            .run::<$t>(driver, output, prog)
                            .await
                    })+
                }
            };
        }
        run!(Answer, Article, Collection, Column, Question, Pin, User)
    }
}
