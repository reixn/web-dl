use crate::progress::Reporter;
use std::{
    fmt::Display,
    fs, io,
    path::{Path, PathBuf},
};

pub mod spec;
use spec::ConfValue;
pub use spec::Manifest;

pub mod leaf;

pub mod branch;

#[derive(Debug)]
pub enum FsOp {
    CreateDir,
    CreateFile,
    OpenFile,
    Remove,
    RenameFile(PathBuf),
}
impl Display for FsOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateDir => f.write_str("create directory"),
            Self::CreateFile => f.write_str("create file"),
            Self::OpenFile => f.write_str("open file"),
            Self::Remove => f.write_str("remove file"),
            Self::RenameFile(pat) => write!(f, "rename to {} from", pat.display()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to {op} {}",path.display())]
    Fs {
        path: PathBuf,
        op: FsOp,
        #[source]
        source: io::Error,
    },
    #[error("failed to deserialize {}", path.display())]
    LoadRon {
        path: PathBuf,
        #[source]
        source: ron::de::SpannedError,
    },
    #[error("failed to serialize {}", path.display())]
    StoreRon {
        path: PathBuf,
        #[source]
        source: ron::Error,
    },
    #[error("failed to get leaf")]
    Leaf(
        #[source]
        #[from]
        leaf::Error,
    ),
    #[error("failed to create symlinks")]
    Link(
        #[source]
        #[from]
        branch::Error,
    ),
}

const LEAVES_DIR: &str = ".leaves";
const LEAVES_FILE: &str = "zhihu.com.ron";

fn load_leaves<P: AsRef<Path>>(path: P) -> Result<spec::ManifestLeaf, Error> {
    let mut path = path.as_ref().join(LEAVES_DIR);
    path.push(LEAVES_FILE);
    if path.exists() {
        ron::de::from_reader(io::BufReader::new(fs::File::open(&path).map_err(|e| {
            Error::Fs {
                path: path.clone(),
                op: FsOp::OpenFile,
                source: e,
            }
        })?))
        .map_err(|e| Error::LoadRon {
            path: path.clone(),
            source: e,
        })
    } else {
        Ok(Default::default())
    }
}
fn save_leaves<P: AsRef<Path>>(path: P, leaves: &spec::ManifestLeaf) -> Result<(), Error> {
    let mut path = path.as_ref().join(LEAVES_DIR);
    if path.exists() {
        path.push(LEAVES_FILE);
        if path.exists() {
            let renamed = path.with_file_name("zhihu.com.old.ron");
            if renamed.exists() {
                fs::remove_file(&renamed).map_err(|e| Error::Fs {
                    path: renamed.clone(),
                    op: FsOp::Remove,
                    source: e,
                })?;
            }
            fs::rename(&path, &renamed).map_err(|e| Error::Fs {
                path: path.to_path_buf(),
                op: FsOp::RenameFile(renamed.clone()),
                source: e,
            })?;
        }
    } else {
        fs::create_dir_all(&path).map_err(|e| Error::Fs {
            path: path.clone(),
            op: FsOp::CreateDir,
            source: e,
        })?;
        path.push(LEAVES_FILE);
    }
    ron::ser::to_writer_pretty(
        io::BufWriter::new(fs::File::create(&path).map_err(|e| Error::Fs {
            path: path.to_path_buf(),
            op: FsOp::CreateFile,
            source: e,
        })?),
        leaves,
        Default::default(),
    )
    .map_err(|e| Error::StoreRon {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

impl super::Driver {
    pub async fn update_manifest<P: Reporter, Pat: AsRef<Path>>(
        &mut self,
        prog: &P,
        manifest: &spec::Manifest,
        dest: Pat,
    ) -> Result<(), Error> {
        let leaves = manifest.merged_leaf();
        self.apply_manifest_leaf(prog, &leaves)
            .await
            .map_err(Error::from)?;
        save_leaves(dest.as_ref(), &leaves)?;
        self.link_manifest(prog, manifest, dest)
            .map_err(Error::from)
    }
    pub async fn apply_manifest<P: Reporter, Pat: AsRef<Path>>(
        &mut self,
        prog: &P,
        manifest: &spec::Manifest,
        dest: Pat,
    ) -> Result<(), Error> {
        let leaves = manifest.merged_leaf();
        self.apply_manifest_leaf(prog, &{
            let mut missing = leaves.clone();
            missing.diff(&load_leaves(dest.as_ref())?);
            missing
        })
        .await
        .map_err(Error::from)?;
        save_leaves(dest.as_ref(), &manifest.merged_leaf())?;
        self.link_manifest(prog, manifest, dest)
            .map_err(Error::from)
    }
}
