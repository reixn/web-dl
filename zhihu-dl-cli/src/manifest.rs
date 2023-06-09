use crate::types::Output;
use anyhow::Context;
use clap::Subcommand;
use std::{
    fs,
    io::{self, Seek},
    path::PathBuf,
};
use termcolor::Color;
use zhihu_dl::{
    driver::{manifest::Manifest, Driver},
    progress::{progress_bar::ProgressReporter, OtherJob, Reporter},
};

#[derive(Debug, Subcommand)]
pub enum ManifestCmd {
    Apply {
        #[arg(default_value = "manifest.ron")]
        path: String,
    },
    Format {
        #[arg(default_value = "manifest.ron")]
        path: String,
    },
    Update {
        #[arg(default_value = "manifest.ron")]
        path: String,
    },
    Link {
        #[arg(default_value = "manifest.ron")]
        path: String,
    },
}

fn load_manif(path: &String) -> anyhow::Result<Manifest> {
    ron::de::from_reader(io::BufReader::new(
        fs::File::open(path.as_str()).with_context(|| format!("failed to open file {}", path))?,
    ))
    .context("failed to deserialize ron")
}
fn current_dir() -> anyhow::Result<PathBuf> {
    std::env::current_dir().context("failed to get current directory")
}
impl ManifestCmd {
    pub async fn run(
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
                let job = reporter.start_job("Applying", format_args!("manifest {}", path));
                driver
                    .apply_manifest(reporter, &load_manif(&path)?, current_dir()?)
                    .await
                    .context("failed to apply manifest")?;
                job.finish("Applied", format_args!("manifest {}", path,));
            }
            Self::Update { path } => {
                let job = reporter.start_job("Updating", format_args!("manifest {}", path));
                driver
                    .update_manifest(reporter, &load_manif(&path)?, current_dir()?)
                    .await
                    .context("failed to update manifest")?;
                job.finish("Updated", format_args!("manifest {}", path));
            }
            Self::Link { path } => {
                let job = reporter.start_job(
                    "Creating",
                    format_args!("symbol links according to manifest {}", path),
                );
                driver
                    .link_manifest(reporter, &load_manif(&path)?, current_dir()?)
                    .with_context(|| {
                        format!("failed to create symbol links according to {}", path)
                    })?;
                job.finish(
                    "Created",
                    format_args!("symbol links according to {}", path),
                );
            }
        }
        Ok(())
    }
}
