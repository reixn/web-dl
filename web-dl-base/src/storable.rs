use crate::id::HasId;
use std::{
    error,
    fmt::Display,
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum IoErrorOp {
    CreateFile,
    OpenFile,
    ReadFile,
    WriteFile,
    CreateDir,
    ReadDir,
    DirEntry,
    Other(&'static str),
}

#[derive(Debug)]
pub enum Error {
    Io {
        op: IoErrorOp,
        path: PathBuf,
        source: io::Error,
    },
    Yaml(serde_yaml::Error),
    Json(serde_json::Error),
    Chained {
        field: String,
        source: Box<Error>,
    },
}
impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io { op, path, .. } => f.write_fmt(format_args!(
                "failed to {} {}",
                match op {
                    IoErrorOp::CreateFile => "create file",
                    IoErrorOp::OpenFile => "open file",
                    IoErrorOp::ReadFile => "read file",
                    IoErrorOp::WriteFile => "write file",
                    IoErrorOp::CreateDir => "create directory",
                    IoErrorOp::ReadDir => "read dir",
                    IoErrorOp::DirEntry => "get entry in",
                    IoErrorOp::Other(op) => op,
                },
                path.display()
            )),
            Error::Yaml(_) => f.write_str("failed to process yaml"),
            Error::Json(_) => f.write_str("failed to process json"),
            Error::Chained { field, .. } => {
                f.write_fmt(format_args!("failed to process field {}", field))
            }
        }
    }
}
impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            Error::Yaml(e) => Some(e),
            Error::Json(e) => Some(e),
            Error::Chained { source, .. } => Some(source),
        }
    }
}
pub fn create_dir_missing(path: &Path) -> Result<(), Error> {
    if !path.exists() {
        fs::create_dir_all(path).map_err(|e| Error::Io {
            op: IoErrorOp::CreateDir,
            path: path.to_path_buf(),
            source: e,
        })
    } else {
        Ok(())
    }
}
pub fn create_file<P: AsRef<Path>>(path: P) -> Result<fs::File, Error> {
    fs::File::create(path.as_ref()).map_err(|e| Error::Io {
        op: IoErrorOp::CreateFile,
        path: path.as_ref().to_path_buf(),
        source: e,
    })
}
pub fn open_file<P: AsRef<Path>>(path: P) -> Result<fs::File, Error> {
    fs::File::open(path.as_ref()).map_err(|e| Error::Io {
        op: IoErrorOp::OpenFile,
        path: path.as_ref().to_path_buf(),
        source: e,
    })
}
pub fn load_yaml<D: serde::de::DeserializeOwned, P: AsRef<Path>>(path: P) -> Result<D, Error> {
    serde_yaml::from_reader::<_, D>(io::BufReader::new(open_file(path)?)).map_err(Error::Yaml)
}
pub fn load_json<D: serde::de::DeserializeOwned, P: AsRef<Path>>(path: P) -> Result<D, Error> {
    serde_json::from_reader::<_, D>(io::BufReader::new(open_file(path)?)).map_err(Error::Json)
}
pub fn store_yaml<D: serde::Serialize, P: AsRef<Path>>(value: &D, path: P) -> Result<(), Error> {
    serde_yaml::to_writer(io::BufWriter::new(create_file(path)?), value).map_err(Error::Yaml)
}
pub fn store_json<D: serde::Serialize, P: AsRef<Path>>(value: &D, path: P) -> Result<(), Error> {
    serde_json::to_writer_pretty(io::BufWriter::new(create_file(path)?), value).map_err(Error::Json)
}
pub fn push_path<P: AsRef<Path>>(path: P, value: &str) -> PathBuf {
    let mut ret = path.as_ref().to_path_buf();
    ret.push(value);
    ret
}

pub mod macro_export {
    pub use std::{self, convert::AsRef, path::Path, result::Result, string::String};
}

#[derive(Debug, Clone, Copy)]
pub struct LoadOpt {
    pub load_raw: bool,
}
impl Default for LoadOpt {
    fn default() -> Self {
        LoadOpt { load_raw: false }
    }
}

pub trait Storable: Sized {
    fn load<P: AsRef<Path>>(path: P, load_opt: LoadOpt) -> Result<Self, Error>;
    fn store<P: AsRef<Path>>(&self, path: P) -> Result<(), Error>;
}
pub fn load_chained<S: Storable, P: AsRef<Path>, C: Display>(
    path: P,
    load_opt: LoadOpt,
    context: C,
) -> Result<S, Error> {
    S::load(path, load_opt).map_err(|e| Error::Chained {
        field: context.to_string(),
        source: Box::new(e),
    })
}
pub fn store_chained<S: Storable, P: AsRef<Path>, C: Display>(
    value: &S,
    path: P,
    context: C,
) -> Result<(), Error> {
    value.store(path).map_err(|e| Error::Chained {
        field: context.to_string(),
        source: Box::new(e),
    })
}
pub use web_dl_derive::Storable;

impl Storable for String {
    fn load<P: AsRef<Path>>(path: P, _: LoadOpt) -> Result<Self, Error> {
        let path = path.as_ref();
        fs::read_to_string(path).map_err(|e| Error::Io {
            op: IoErrorOp::ReadFile,
            path: path.to_owned(),
            source: e,
        })
    }
    fn store<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        let path = path.as_ref();
        fs::write(path, self).map_err(|e| Error::Io {
            op: IoErrorOp::WriteFile,
            path: path.to_owned(),
            source: e,
        })
    }
}
impl<I: Storable> Storable for Option<I> {
    fn load<P: AsRef<Path>>(path: P, load_opt: LoadOpt) -> Result<Self, Error> {
        let path = path.as_ref();
        if path.exists() {
            Ok(Some(I::load(path, load_opt)?))
        } else {
            Ok(None)
        }
    }
    fn store<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        match self {
            Some(i) => i.store(path),
            None => Ok(()),
        }
    }
}
impl Storable for serde_json::Value {
    fn load<P: AsRef<Path>>(path: P, _: LoadOpt) -> Result<Self, Error> {
        load_json(path)
    }
    fn store<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        store_json(self, path)
    }
}
impl<I: HasId + Storable> Storable for Vec<I> {
    fn load<P: AsRef<Path>>(path: P, load_opt: LoadOpt) -> Result<Self, Error> {
        let mut ret = Vec::new();
        let path = path.as_ref();
        for c in fs::read_dir(path).map_err(|e| Error::Io {
            op: IoErrorOp::ReadDir,
            path: path.to_path_buf(),
            source: e,
        })? {
            let c = c.map_err(|e| Error::Io {
                op: IoErrorOp::DirEntry,
                path: path.to_path_buf(),
                source: e,
            })?;
            let path = path.with_file_name(c.path());
            ret.push(I::load(path, load_opt).map_err(|e| Error::Chained {
                field: c.path().display().to_string(),
                source: Box::new(e),
            })?);
        }
        Ok(ret)
    }
    fn store<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        let path = path.as_ref();
        create_dir_missing(path)?;
        for i in self {
            let id = i.id().to_string();
            i.store(push_path(path, id.as_str()))
                .map_err(|e| Error::Chained {
                    field: id,
                    source: Box::new(e),
                })?;
        }
        Ok(())
    }
}
