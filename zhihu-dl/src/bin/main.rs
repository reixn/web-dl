#![feature(format_args_nl)]

use anyhow::Context;
use clap::{Args, FromArgMatches, Parser, Subcommand, ValueEnum};
use std::{
    fmt::{self, Display},
    io::Write,
    path::PathBuf,
};
use termcolor::{BufferedStandardStream, Color, ColorSpec, WriteColor};
use web_dl_base::{id::HasId, media};
use zhihu_dl::{
    driver::Driver,
    item::{
        answer::{Answer, AnswerId},
        any::Any,
        article::{Article, ArticleId},
        collection::{Collection, CollectionId},
        column::{Column, ColumnId, ColumnItem},
        pin::{Pin, PinId},
        question::{Question, QuestionId},
        user::{self, User, UserId},
        Fetchable, Item, ItemContainer, VoidOpt,
    },
    progress::{progress_bar::ProgressReporter, Reporter},
    store,
};

struct Output {
    progress_bar: indicatif::MultiProgress,
    buffer: BufferedStandardStream,
}
#[allow(unused_must_use)]
impl Output {
    fn write_tagged(&mut self, color: Color, tag: &str, fmt: fmt::Arguments<'_>) {
        self.progress_bar.suspend(|| {
            self.buffer.set_color(ColorSpec::new().set_fg(Some(color)));
            self.buffer.write_fmt(format_args!("{:>13} ", tag));
            self.buffer.reset();
            self.buffer.write_fmt(fmt);
            self.buffer.flush();
        })
    }
    fn write_error(&mut self, error: anyhow::Error) {
        self.progress_bar.suspend(|| {
            self.buffer
                .set_color(ColorSpec::new().set_fg(Some(Color::Red)));
            self.buffer.write(b"error: ");
            self.buffer.reset();
            writeln!(&mut self.buffer, "{:?}", error);
            self.buffer.flush();
        })
    }
    fn write_warn(&mut self, fmt: fmt::Arguments<'_>) {
        self.progress_bar.suspend(|| {
            self.buffer
                .set_color(ColorSpec::new().set_fg(Some(Color::Yellow)));
            self.buffer.write(b"warning: ");
            self.buffer.reset();
            self.buffer.write_fmt(fmt);
            self.buffer.flush();
        })
    }
}

#[derive(Debug, Args)]
struct GetOpt {
    #[arg(long)]
    comments: bool,
}
impl Display for GetOpt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.comments {
            "with comments"
        } else {
            "no comments"
        })
    }
}

#[derive(Debug, Args)]
struct LinkOpt {
    #[arg(long)]
    link_absolute: bool,
    #[arg(value_hint = clap::ValueHint::AnyPath)]
    dest: String,
}

#[derive(Debug, Subcommand)]
enum ItemOper {
    /// download and add to store, but not link
    Get {
        #[command(flatten)]
        get_opt: GetOpt,
    },
    Download {
        #[command(flatten)]
        get_opt: GetOpt,
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        link_opt: LinkOpt,
    },
    Update {
        #[command(flatten)]
        get_opt: GetOpt,
    },
}
impl ItemOper {
    async fn run<I>(
        self,
        driver: &mut Driver,
        output: &mut Output,
        id: <I as HasId>::Id<'_>,
        prog: &ProgressReporter,
    ) -> Result<(), anyhow::Error>
    where
        I: Fetchable + Item + media::HasImage + store::BasicStoreItem,
    {
        if !driver.is_initialized() {
            anyhow::bail!("client is not initialized");
        }
        let (start_tag, ok_tag, name, opt) = match &self {
            ItemOper::Get { get_opt } => ("Getting", "Got", "get", format!("({})", get_opt)),
            ItemOper::Download {
                get_opt,
                name,
                link_opt,
            } => ("Downloading", "Downloaded", "download", {
                let mut parent = PathBuf::from(link_opt.dest.as_str());
                match &name {
                    Some(n) => parent.push(n.as_str()),
                    None => parent.push(id.to_string()),
                };
                format!(
                    "({}) to {}[{}]",
                    get_opt,
                    parent.display(),
                    if link_opt.link_absolute {
                        "link absolute"
                    } else {
                        "link relative"
                    }
                )
            }),
            ItemOper::Update { get_opt } => {
                ("Updating", "Updated", "update", format!("({})", get_opt))
            }
        };
        output.write_tagged(
            Color::Cyan,
            start_tag,
            format_args_nl!("{item} {id} {opt}", item = I::TYPE, id = id, opt = opt),
        );
        match self {
            ItemOper::Get { get_opt } => driver
                .get_item::<I, _>(&prog.start_item(I::TYPE, id), id, get_opt.comments)
                .await
                .map(|_| ()),
            ItemOper::Download {
                get_opt,
                name,
                link_opt,
            } => {
                let id_str = id.to_string();
                driver
                    .download_item::<I, _, _>(
                        &prog.start_item(I::TYPE, id),
                        id,
                        get_opt.comments,
                        !link_opt.link_absolute,
                        PathBuf::from(link_opt.dest.as_str()),
                        name.as_ref().map_or(id_str.as_str(), |v| v.as_str()),
                    )
                    .await
                    .map(|_| ())
            }
            ItemOper::Update { get_opt } => driver
                .update_item::<I, _>(&prog.start_item(I::TYPE, id), id, get_opt.comments)
                .await
                .map(|_| ()),
        }
        .with_context(|| {
            format!(
                "failed to {verb} {item} {id} {opt}",
                verb = name,
                item = I::TYPE,
                id = id,
                opt = opt
            )
        })?;
        output.write_tagged(
            Color::Green,
            ok_tag,
            format_args_nl!("{item} {id} {opt}", item = I::TYPE, id = id, opt = opt),
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
struct UserSpec {
    #[arg(long)]
    id: UserId,
    #[arg(long)]
    url_token: String,
}

#[derive(Debug, Subcommand)]
enum ItemCmd {
    Answer {
        #[arg(long)]
        id: u64,
        #[command(subcommand)]
        operation: ItemOper,
    },
    Article {
        #[arg(long)]
        id: u64,
        #[command(subcommand)]
        operation: ItemOper,
    },
    Collection {
        #[arg(long)]
        id: u64,
        #[command(subcommand)]
        operation: ItemOper,
    },
    Column {
        #[arg(long)]
        id: String,
        #[command(subcommand)]
        operation: ItemOper,
    },
    Pin {
        #[arg(long)]
        id: u64,
        #[command(subcommand)]
        operation: ItemOper,
    },
    Question {
        #[arg(long)]
        id: u64,
        #[command(subcommand)]
        operation: ItemOper,
    },
    User {
        #[command(flatten)]
        user_id: UserSpec,
        #[command(subcommand)]
        operation: ItemOper,
    },
}
impl ItemCmd {
    async fn run(
        self,
        driver: &mut Driver,
        output: &mut Output,
        prog: &ProgressReporter,
    ) -> Result<(), anyhow::Error> {
        macro_rules! run {
            ($( ($t:ident, $i:expr) ),*) => {
                match self {
                    $(ItemCmd::$t { id, operation } => {
                        operation
                            .run::<$t>(driver, output, $i(id), prog)
                            .await
                    })+
                    ItemCmd::Column { id, operation } => {
                        operation
                            .run::<Column>(driver, output, &ColumnId(id), prog)
                            .await
                     }
                    ItemCmd::User { user_id, operation } => {
                        operation
                            .run::<User>(
                                driver,
                                output,
                                user::StoreId(user_id.id, user_id.url_token.as_str()),
                                prog,
                            )
                            .await
                    }
                }
            };
        }
        run!(
            (Answer, AnswerId),
            (Article, ArticleId),
            (Collection, CollectionId),
            (Question, QuestionId),
            (Pin, PinId)
        )
    }
}

#[derive(Debug, Subcommand)]
enum ContainerOper {
    Get {
        #[command(flatten)]
        get_opt: GetOpt,
    },
    Download {
        #[command(flatten)]
        get_opt: GetOpt,
        #[command(flatten)]
        link_opt: LinkOpt,
    },
}
impl ContainerOper {
    async fn run<IC, I, O>(
        self,
        driver: &mut Driver,
        output: &mut Output,
        prog: &ProgressReporter,
        id: <IC as HasId>::Id<'_>,
        option: O,
    ) -> Result<(), anyhow::Error>
    where
        I: Item + media::HasImage + store::StoreItem,
        O: Display + Copy,
        IC: ItemContainer<I, O>,
    {
        if !driver.is_initialized() {
            anyhow::bail!("client is not initialized");
        }
        let (start_tag, ok_tag, name, opt) = match &self {
            Self::Get { get_opt } => ("Getting", "Got", "get", format!("({})", get_opt)),
            Self::Download { get_opt, link_opt } => (
                "Downloading",
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
                con_id = id,
                oper_opt = opt
            ),
        );
        let v = match self {
            Self::Get { get_opt } => {
                driver
                    .get_container::<IC, I, O, _>(
                        &prog.start_item_container(IC::TYPE, id, I::TYPE),
                        id,
                        option,
                        get_opt.comments,
                    )
                    .await
            }
            Self::Download { get_opt, link_opt } => {
                driver
                    .download_container::<IC, I, O, _, _>(
                        &prog.start_item_container(IC::TYPE, id, I::TYPE),
                        id,
                        option,
                        get_opt.comments,
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
                con_id = id,
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
                con_id = id,
                oper_opt = opt
            ),
        );
        Ok(())
    }
}

#[derive(Debug, Subcommand)]
enum CollectionEntry {
    Item {
        #[command(subcommand)]
        operation: ContainerOper,
    },
}

#[derive(Debug, Subcommand)]
enum ColumnEntry {
    Item {
        #[arg(long)]
        pinned: bool,
        #[command(subcommand)]
        operation: ContainerOper,
    },
}
#[derive(Debug, Subcommand)]
enum QuestionEntry {
    Answer {
        #[command(subcommand)]
        operation: ContainerOper,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CollectionTyp {
    Liked,
    Created,
}
#[derive(Debug, Subcommand)]
enum UserEntry {
    Answer {
        #[command(subcommand)]
        operation: ContainerOper,
    },
    Article {
        #[command(subcommand)]
        operation: ContainerOper,
    },
    Column {
        #[command(subcommand)]
        operation: ContainerOper,
    },
    Collection {
        #[arg(long = "type")]
        typ: CollectionTyp,
        #[command(subcommand)]
        operation: ContainerOper,
    },
    Pin {
        #[command(subcommand)]
        operation: ContainerOper,
    },
}
#[derive(Debug, Subcommand)]
enum ContainerCmd {
    Collection {
        #[arg(long)]
        id: u64,
        #[command(subcommand)]
        operation: CollectionEntry,
    },
    Column {
        #[arg(long)]
        id: String,
        #[command(subcommand)]
        operation: ColumnEntry,
    },
    User {
        #[command(flatten)]
        user_id: UserSpec,
        #[command(subcommand)]
        operation: UserEntry,
    },
}
impl ContainerCmd {
    async fn run(
        self,
        driver: &mut Driver,
        output: &mut Output,
        prog: &ProgressReporter,
    ) -> Result<(), anyhow::Error> {
        match self {
            Self::Collection {
                id,
                operation: CollectionEntry::Item { operation },
            } => {
                operation
                    .run::<Collection, Any, _>(driver, output, prog, CollectionId(id), VoidOpt)
                    .await
            }
            Self::Column {
                id,
                operation: ColumnEntry::Item { pinned, operation },
            } => {
                operation
                    .run::<Column, Any, _>(
                        driver,
                        output,
                        prog,
                        &ColumnId(id),
                        if pinned {
                            ColumnItem::Pinned
                        } else {
                            ColumnItem::Regular
                        },
                    )
                    .await
            }
            Self::User { user_id, operation } => {
                let user_id = user::StoreId(user_id.id, user_id.url_token.as_str());
                match operation {
                    UserEntry::Answer { operation } => {
                        operation
                            .run::<User, Answer, _>(driver, output, prog, user_id, VoidOpt)
                            .await
                    }
                    UserEntry::Article { operation } => {
                        operation
                            .run::<User, Article, _>(driver, output, prog, user_id, VoidOpt)
                            .await
                    }
                    UserEntry::Collection { typ, operation } => {
                        operation
                            .run::<User, Collection, _>(
                                driver,
                                output,
                                prog,
                                user_id,
                                match typ {
                                    CollectionTyp::Created => user::CollectionOpt::Created,
                                    CollectionTyp::Liked => user::CollectionOpt::Liked,
                                },
                            )
                            .await
                    }
                    UserEntry::Column { operation } => {
                        operation
                            .run::<User, Column, _>(driver, output, prog, user_id, VoidOpt)
                            .await
                    }
                    UserEntry::Pin { operation } => {
                        operation
                            .run::<User, Pin, _>(driver, output, prog, user_id, VoidOpt)
                            .await
                    }
                }
            }
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    Init,
    Item {
        #[command(subcommand)]
        cmd: ItemCmd,
    },
    Container {
        #[command(subcommand)]
        cmd: ContainerCmd,
    },
    Save,
    Exit {
        #[arg(short, long)]
        /// force exit, ignore error
        force: bool,
    },
}
fn save_state(driver: &mut Driver, output: &mut Output) -> Result<(), anyhow::Error> {
    driver.save().context("failed to save store state")?;
    output.write_tagged(Color::Blue, "Saved", format_args_nl!("store state"));
    Ok(())
}
async fn init_driver(driver: &mut Driver, output: &mut Output) -> Result<(), anyhow::Error> {
    output.write_tagged(
        Color::Cyan,
        "Initializing",
        format_args_nl!("initializing driver"),
    );
    driver.init().await.context("failed to init driver")?;
    output.write_tagged(Color::Blue, "Ready", format_args_nl!("Initialized driver"));
    Ok(())
}
impl Command {
    async fn run(
        self,
        driver: &mut Driver,
        output: &mut Output,
        prog: &ProgressReporter,
    ) -> Result<bool, anyhow::Error> {
        let st = std::time::SystemTime::now();
        match self {
            Self::Init => init_driver(driver, output).await?,
            Self::Item { cmd } => cmd.run(driver, output, prog).await?,
            Self::Container { cmd } => cmd.run(driver, output, prog).await?,
            Self::Save => save_state(driver, output)?,
            Self::Exit { force } => {
                if driver.store.is_dirty() {
                    match save_state(driver, output) {
                        Ok(_) => return Ok(true),
                        Err(e) => {
                            if force {
                                output.write_warn(format_args_nl!("{:?}", e));
                                return Ok(true);
                            } else {
                                return Ok(false);
                            }
                        }
                    }
                } else {
                    return Ok(true);
                }
            }
        }
        output.write_tagged(
            Color::Green,
            "Finished",
            format_args_nl!(
                "Command finished after {}",
                indicatif::HumanDuration(std::time::SystemTime::now().duration_since(st).unwrap())
            ),
        );
        Ok(false)
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Verbosity {
    Critical,
    Error,
    Warning,
    Info,
    Debug,
    Trace,
    Off,
}
#[derive(Debug, Parser)]
#[command(name = "zhihu-dl", about, version)]
struct Cli {
    #[arg(long, default_value = ".store")]
    store_path: String,
    #[arg(long, short)]
    verbosity: Option<Verbosity>,
    #[command(subcommand)]
    command: Option<Command>,
}

struct LogDrain<D: slog::Drain> {
    progress_bar: indicatif::MultiProgress,
    term: D,
}
impl<D: slog::Drain> slog::Drain for LogDrain<D> {
    type Ok = D::Ok;
    type Err = D::Err;
    fn log(
        &self,
        record: &slog::Record,
        values: &slog::OwnedKVList,
    ) -> std::result::Result<Self::Ok, Self::Err> {
        self.progress_bar.suspend(|| self.term.log(record, values))
    }
}

async fn run_cli(
    reporter: &ProgressReporter,
    output: &mut Output,
    cli: Cli,
) -> Result<(), anyhow::Error> {
    let mut driver = {
        let p = PathBuf::from(cli.store_path.as_str());
        if p.exists() {
            let d = Driver::open(p.as_path())
                .with_context(|| format!("failed to open store as {}", p.display()))?;
            output.write_tagged(
                Color::Blue,
                "Opened",
                format_args_nl!("store at {}", p.display()),
            );
            d
        } else {
            let d = Driver::create(p.as_path())
                .with_context(|| format!("failed to create store at {}", p.display()))?;
            output.write_tagged(
                Color::Blue,
                "Created",
                format_args_nl!("new store at {}", p.display()),
            );
            d
        }
    };

    if let Some(v) = cli.command {
        init_driver(&mut driver, output).await?;
        v.run(&mut driver, output, &reporter).await?;
        return save_state(&mut driver, output);
    }
    let mut editor = rustyline::Editor::<(), _>::with_history(
        rustyline::Config::builder()
            .auto_add_history(true)
            .max_history_size(3000)
            .unwrap()
            .build(),
        rustyline::history::MemHistory::new(),
    )
    .context("failed to create editor")?;
    loop {
        let input = match editor
            .readline("zhihu-dl > ")
            .map_err(anyhow::Error::new)
            .and_then(|s| shlex::split(s.as_str()).context("erroneous quoting"))
        {
            Ok(i) => i,
            Err(e) => {
                output.write_error(e);
                continue;
            }
        };
        match Command::augment_subcommands(clap::Command::new("repl").multicall(true))
            .subcommand_required(true)
            .try_get_matches_from(input.into_iter())
            .and_then(|am| Command::from_arg_matches(&am))
        {
            Ok(cmd) => match cmd.run(&mut driver, output, reporter).await {
                Ok(true) => break,
                Ok(false) => (),
                Err(e) => {
                    output.write_error(e);
                }
            },
            Err(e) => output
                .progress_bar
                .suspend(|| println!("{}", e.render().ansi())),
        }
    }
    Ok(())
}

fn main() {
    use slog::Drain;
    let cmd = Cli::parse();
    let reporter = ProgressReporter::new(None);

    let log = slog::Logger::root(
        {
            let mut lb = slog_envlogger::LogBuilder::new(std::sync::Mutex::new(
                (LogDrain {
                    progress_bar: reporter.multi_progress.clone(),
                    term: slog_term::FullFormat::new(
                        slog_term::TermDecorator::new().stdout().build(),
                    )
                    .build()
                    .fuse(),
                })
                .fuse(),
            ));
            if let Some(v) = cmd.verbosity {
                lb = lb.filter(
                    None,
                    match v {
                        Verbosity::Off => slog::FilterLevel::Off,
                        Verbosity::Critical => slog::FilterLevel::Critical,
                        Verbosity::Error => slog::FilterLevel::Error,
                        Verbosity::Warning => slog::FilterLevel::Warning,
                        Verbosity::Info => slog::FilterLevel::Info,
                        Verbosity::Trace => slog::FilterLevel::Trace,
                        Verbosity::Debug => slog::FilterLevel::Debug,
                    },
                );
            }
            if let Ok(v) = std::env::var("RUST_LOG") {
                lb = lb.parse(v.as_str());
            }
            lb.build().fuse()
        },
        slog::o!(),
    );
    let _scope_guard = slog_scope::set_global_logger(log);
    let _std_log_guard = slog_stdlog::init().unwrap();

    let mut output = Output {
        progress_bar: reporter.multi_progress.clone(),
        buffer: BufferedStandardStream::stdout(termcolor::ColorChoice::Auto),
    };

    if let Err(e) = tokio::runtime::Runtime::new()
        .context("failed to create context")
        .and_then(|rt| rt.block_on(run_cli(&reporter, &mut output, cmd)))
    {
        output.write_error(e);
    }
}
