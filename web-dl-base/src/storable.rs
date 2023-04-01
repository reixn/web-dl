use crate::id::HasId;
use std::{
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
impl Display for IoErrorOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            IoErrorOp::CreateFile => "create file",
            IoErrorOp::OpenFile => "open file",
            IoErrorOp::ReadFile => "read file",
            IoErrorOp::WriteFile => "write file",
            IoErrorOp::CreateDir => "create directory",
            IoErrorOp::ReadDir => "read dir",
            IoErrorOp::DirEntry => "get entry in",
            IoErrorOp::Other(op) => op,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to {op} {}", path.display())]
    Io {
        op: IoErrorOp,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to process yaml")]
    Yaml(
        #[source]
        #[from]
        serde_yaml::Error,
    ),
    #[error("failed to process json")]
    Json(
        #[source]
        #[from]
        serde_json::Error,
    ),
    #[error("failed to process field {field}")]
    Chained {
        field: String,
        #[source]
        source: Box<Error>,
    },
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LoadOpt {
    pub load_raw: bool,
}

pub trait Storable: Sized {
    fn load<P: AsRef<Path>>(path: P, load_opt: LoadOpt) -> Result<Self, Error>;
    fn store<P: AsRef<Path>>(&self, path: P) -> Result<(), Error>;
}

#[doc(hidden)]
/// private module, for derive macro only
pub mod macro_export {
    use super::{Error, IoErrorOp, LoadOpt, Storable};
    pub use std::{
        self, convert::AsRef, default::Default, path::Path, result::Result, string::String,
    };
    use std::{fmt::Display, fs, io};

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
    pub fn store_yaml<D: serde::Serialize, P: AsRef<Path>>(
        value: &D,
        path: P,
    ) -> Result<(), Error> {
        serde_yaml::to_writer(io::BufWriter::new(create_file(path)?), value).map_err(Error::Yaml)
    }
    pub fn store_json<D: serde::Serialize, P: AsRef<Path>>(
        value: &D,
        path: P,
    ) -> Result<(), Error> {
        serde_json::to_writer_pretty(io::BufWriter::new(create_file(path)?), value)
            .map_err(Error::Json)
    }
}
use macro_export::create_dir_missing;

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
impl Storable for serde_json::Value {
    fn load<P: AsRef<Path>>(path: P, _: LoadOpt) -> Result<Self, Error> {
        macro_export::load_json(path)
    }
    fn store<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        macro_export::store_json(self, path)
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
            i.store(path.join(id.as_str()))
                .map_err(|e| Error::Chained {
                    field: id,
                    source: Box::new(e),
                })?;
        }
        Ok(())
    }
}
