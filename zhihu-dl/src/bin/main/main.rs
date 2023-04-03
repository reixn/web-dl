#![feature(format_args_nl)]

use anyhow::Context;
use clap::{FromArgMatches, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use termcolor::{BufferedStandardStream, Color};
use zhihu_dl::{
    driver::Driver,
    progress::{progress_bar::ProgressReporter, Reporter},
};

mod container;
mod item;
mod types;

use container::ContainerCmd;
use item::ItemCmd;
use types::*;

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
        v.run(&mut driver, output, reporter).await?;
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
    slog_stdlog::init().unwrap();

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
