#![feature(format_args_nl)]
#![feature(try_blocks)]

use anyhow::Context;
use clap::{FromArgMatches, Parser, Subcommand, ValueEnum};
use indicatif::HumanDuration;
use std::{
    fs,
    io::{self, Seek},
    path::PathBuf,
    time::SystemTime,
};
use termcolor::{BufferedStandardStream, Color};
use zhihu_dl::{
    driver::{manifest::Manifest, Driver},
    progress::{progress_bar::ProgressReporter, Reporter},
    store,
};

mod container;
mod item;
mod types;

use container::ContainerCmd;
use item::ItemCmd;
use types::*;

#[derive(Debug, Subcommand)]
enum ManifestOper {
    Apply {
        #[arg(default_value = "manifest.ron")]
        path: String,
    },
    Format {
        #[arg(default_value = "manifest.ron")]
        path: String,
    },
}
impl ManifestOper {
    async fn run(
        self,
        reporter: &ProgressReporter,
        output: &mut Output,
        driver: &mut Driver,
    ) -> Result<(), anyhow::Error> {
        match self {
            Self::Format { path } => {
                let mut file = fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(path.as_str())
                    .with_context(|| format!("failed to open file {}", path))?;
                let m: Manifest = ron::de::from_reader(io::BufReader::new(&file))
                    .context("failed to deserialize ron")?;
                file.set_len(0)
                    .context("failed to truncate file for write")?;
                file.rewind().context("failed to seek to begin")?;
                ron::ser::to_writer_pretty(
                    io::BufWriter::new(&file),
                    &m,
                    ron::ser::PrettyConfig::default(),
                )
                .context("failed to serialize ron")?;
                output.write_tagged(
                    Color::Green,
                    "Formatted",
                    format_args_nl!("ron manifest {}", path),
                );
            }
            Self::Apply { path } => {
                let st = SystemTime::now();
                output.write_tagged(
                    Color::Cyan,
                    "Applying",
                    format_args_nl!("manifest {}", path),
                );
                driver
                    .apply_manifest(
                        reporter,
                        &ron::de::from_reader(io::BufReader::new(
                            fs::File::open(path.as_str())
                                .with_context(|| format!("failed to open file {}", path))?,
                        ))
                        .context("failed to deserialize ron")?,
                    )
                    .await
                    .context("failed to apply manifest")?;
                output.write_tagged(
                    Color::Green,
                    "Applied",
                    format_args_nl!(
                        "manifest {} took {}",
                        path,
                        HumanDuration(SystemTime::now().duration_since(st).unwrap())
                    ),
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// init client
    Init,
    Item {
        #[command(subcommand)]
        cmd: ItemCmd,
    },
    Container {
        #[command(subcommand)]
        cmd: ContainerCmd,
    },
    Command {
        file: String,
    },
    Manifest {
        #[command(subcommand)]
        operation: ManifestOper,
    },
    /// migrate store
    Migrate,
    /// save store state
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
    let st = std::time::SystemTime::now();
    output.write_tagged(
        Color::Cyan,
        "Initializing",
        format_args_nl!("initializing driver"),
    );
    driver.init().await.context("failed to init driver")?;
    output.write_tagged(
        Color::Blue,
        "Ready",
        format_args_nl!(
            "Initialized driver took {}",
            HumanDuration(std::time::SystemTime::now().duration_since(st).unwrap())
        ),
    );
    Ok(())
}
impl Command {
    fn parse_line(line: Vec<String>) -> clap::error::Result<Self> {
        Self::augment_subcommands(clap::Command::new("repl").multicall(true))
            .subcommand_required(true)
            .try_get_matches_from(line.into_iter())
            .and_then(|am| Self::from_arg_matches(&am))
    }
    fn run(
        self,
        runtime: &tokio::runtime::Runtime,
        driver: &mut Driver,
        output: &mut Output,
        prog: &ProgressReporter,
    ) -> Result<bool, anyhow::Error> {
        match self {
            Self::Init => runtime.block_on(init_driver(driver, output))?,
            Self::Item { cmd } => runtime.block_on(cmd.run(driver, prog))?,
            Self::Container { cmd } => runtime.block_on(cmd.run(driver, prog))?,
            Self::Save => save_state(driver, output)?,
            Self::Command { file } => {
                output.write_tagged(
                    Color::Cyan,
                    "Running",
                    format_args_nl!("commands in {}", file),
                );
                let st = std::time::SystemTime::now();
                for (idx, s) in fs::read_to_string(&file)
                    .with_context(|| format!("failed to read {}", file))?
                    .lines()
                    .enumerate()
                {
                    if s.trim().is_empty() {
                        continue;
                    }
                    Self::parse_line(
                        shlex::split(s)
                            .with_context(|| format!("{}:{}: erroneous quoting", file, idx + 1))?,
                    )
                    .with_context(|| format!("{}:{}: failed to parse command", file, idx + 1))?
                    .run(runtime, driver, output, prog)?;
                }
                output.write_tagged(
                    Color::Green,
                    "Completed",
                    format_args_nl!(
                        "running commands in {} took {}",
                        file,
                        HumanDuration(std::time::SystemTime::now().duration_since(st).unwrap())
                    ),
                )
            }
            Self::Manifest { operation } => {
                runtime.block_on(operation.run(prog, output, driver))?
            }
            Self::Migrate => anyhow::bail!("migrate is not supported in repl or file"),
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
    #[arg(long)]
    /// don't init client on start
    no_init: bool,
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

fn run_cli(
    reporter: &ProgressReporter,
    output: &mut Output,
    cli: Cli,
) -> Result<(), anyhow::Error> {
    if let Some(Command::Migrate) = cli.command {
        store::Store::migrate(cli.store_path).context("failed to migrate store")?;
        output.write_tagged(Color::Green, "Success", format_args_nl!("migrated store"));
        return Ok(());
    }
    let runtime = tokio::runtime::Runtime::new().context("failed to create runtime")?;
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
    if !cli.no_init {
        runtime.block_on(init_driver(&mut driver, output))?;
    }

    if let Some(v) = cli.command {
        v.run(&runtime, &mut driver, output, reporter)?;
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
        match Command::parse_line(input) {
            Ok(cmd) => match cmd.run(&runtime, &mut driver, output, reporter) {
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
        std::sync::Mutex::new({
            let mut lb = slog_envlogger::LogBuilder::new(
                (LogDrain {
                    progress_bar: reporter.multi_progress.clone(),
                    term: slog_term::FullFormat::new(
                        slog_term::TermDecorator::new().stdout().build(),
                    )
                    .build()
                    .fuse(),
                })
                .fuse(),
            );
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
            } else {
                lb = lb.filter(None, slog::FilterLevel::Warning);
            }
            if let Ok(v) = std::env::var("RUST_LOG") {
                lb = lb.parse(v.as_str());
            }
            lb.build().fuse()
        })
        .fuse(),
        slog::o!(),
    );
    let _scope_guard = slog_scope::set_global_logger(log);
    slog_stdlog::init().unwrap();

    let mut output = Output {
        progress_bar: reporter.multi_progress.clone(),
        buffer: BufferedStandardStream::stdout(termcolor::ColorChoice::Auto),
    };

    if let Err(e) = run_cli(&reporter, &mut output, cmd) {
        output.write_error(e);
    }
}
