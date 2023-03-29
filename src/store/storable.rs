use crate::id::{self, HasId};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    error,
    fmt::Display,
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub(crate) enum ErrorOp {
    Load,
    Store,
}
#[derive(Debug)]
pub(crate) enum ErrorSource {
    Io(io::Error),
    Yaml(serde_yaml::Error),
    Json(serde_json::Error),
    Chained(Box<Error>),
}
#[derive(Debug)]
pub struct Error {
    pub(crate) op: ErrorOp,
    pub(crate) context: String,
    pub(crate) source: ErrorSource,
}
impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "failed to {} {}",
            match &self.op {
                ErrorOp::Load => "load",
                ErrorOp::Store => "store",
            },
            self.context
        ))
    }
}
impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self.source {
            ErrorSource::Io(e) => Some(e),
            ErrorSource::Yaml(e) => Some(e),
            ErrorSource::Json(e) => Some(e),
            ErrorSource::Chained(e) => Some(e),
        }
    }
}
impl Error {
    pub(crate) fn load_error<C: Display>(context: C, source: ErrorSource) -> Self {
        Self {
            op: ErrorOp::Load,
            context: context.to_string(),
            source,
        }
    }
    pub(crate) fn store_error<C: Display>(context: C, source: ErrorSource) -> Self {
        Self {
            op: ErrorOp::Store,
            context: context.to_string(),
            source,
        }
    }
}
pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn push_path<P: AsRef<Path>>(path: &PathBuf, name: P) -> PathBuf {
    let mut pb = path.clone();
    pb.push(name);
    pb
}
pub(crate) fn load_yaml<D: DeserializeOwned, F: AsRef<str>>(path: &PathBuf, name: F) -> Result<D> {
    let name = name.as_ref();
    serde_yaml::from_reader::<_, D>(
        fs::File::open(push_path(path, name))
            .map_err(|e| Error::load_error(name, ErrorSource::Io(e)))?,
    )
    .map_err(|e| Error::load_error(name, ErrorSource::Yaml(e)))
}
pub(crate) fn store_yaml<D: Serialize, F: AsRef<str>>(
    value: &D,
    path: &PathBuf,
    name: F,
) -> Result<()> {
    let name = name.as_ref();
    serde_yaml::to_writer(
        fs::File::create(push_path(path, name))
            .map_err(|e| Error::store_error(name, ErrorSource::Io(e)))?,
        value,
    )
    .map_err(|e| Error::store_error(name, ErrorSource::Yaml(e)))
}
pub(crate) fn write_file<V: AsRef<[u8]>, F: AsRef<str>>(
    contents: &V,
    path: &PathBuf,
    name: F,
) -> Result<()> {
    let name = name.as_ref();
    fs::write(push_path(path, name), contents)
        .map_err(|e| Error::store_error(name, ErrorSource::Io(e)))
}
pub(crate) fn read_file<F: AsRef<str>>(path: &PathBuf, name: F) -> Result<Vec<u8>> {
    let name = name.as_ref();
    fs::read(push_path(path, name)).map_err(|e| Error::load_error(name, ErrorSource::Io(e)))
}
pub(crate) fn read_text_file<F: AsRef<str>>(path: &PathBuf, name: F) -> Result<String> {
    let name = name.as_ref();
    fs::read_to_string(push_path(path, name))
        .map_err(|e| Error::load_error(name, ErrorSource::Io(e)))
}
pub(crate) fn read_dir<P: AsRef<Path>, C: Display>(path: P, context: C) -> Result<fs::ReadDir> {
    fs::read_dir(path).map_err(|e| Error::load_error(context, ErrorSource::Io(e)))
}
pub(crate) fn dir_entry<C: Display>(
    entry: io::Result<fs::DirEntry>,
    context: C,
) -> Result<fs::DirEntry> {
    entry.map_err(|e| Error::load_error(context, ErrorSource::Io(e)))
}
pub(crate) fn create_dir_missing<P: AsRef<Path>, C: Display>(path: P, context: C) -> Result<()> {
    let path = path.as_ref();
    if path.is_dir() {
        return Ok(());
    }
    fs::create_dir(path).map_err(|e| Error::store_error(context, ErrorSource::Io(e)))
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

pub trait Storable: Sized + HasId {
    fn store<P: AsRef<Path>>(&self, path: P) -> Result<()>;
    fn load<P: AsRef<Path>>(path: P, load_opt: LoadOpt) -> Result<Self>;
}
pub fn store_path<'a, S: Storable>(id: S::Id<'a>, mut parent: PathBuf) -> PathBuf {
    parent.push(id.to_string());
    parent
}
pub(crate) fn load_object<S: Storable, P: AsRef<Path>, C: Display>(
    path: P,
    load_opt: LoadOpt,
    context: C,
) -> Result<S> {
    S::load(path, load_opt)
        .map_err(|e| Error::load_error(context, ErrorSource::Chained(Box::new(e))))
}
pub(crate) fn load_fixed_id_obj<const N: &'static str, S, C>(
    path: PathBuf,
    load_opt: LoadOpt,
    context: C,
) -> Result<S>
where
    S: Storable + for<'a> HasId<Id<'a> = id::Fixed<N>>,
    C: Display,
{
    load_object(store_path::<S>(id::Fixed, path), load_opt, context)
}
pub(crate) fn store_object_to<S: Storable, C: Display, P: AsRef<Path>>(
    value: &S,
    path: P,
    context: C,
) -> Result<()> {
    let path = path.as_ref();
    create_dir_missing(path, "object dir")?;
    value
        .store(path)
        .map_err(|e| Error::store_error(context, ErrorSource::Chained(Box::new(e))))
}
pub(crate) fn store_object<S: Storable, C: Display>(
    value: &S,
    mut path: PathBuf,
    context: C,
) -> Result<()> {
    path.push(value.id().to_string());
    store_object_to(value, path, context)
}

impl<S: Storable> Storable for Vec<S>
where
    Vec<S>: HasId,
{
    fn store<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        for i in self {
            store_object(i, path.clone(), i.id())?;
        }
        Ok(())
    }
    fn load<P: AsRef<Path>>(path: P, load_opt: LoadOpt) -> Result<Self> {
        let mut ret = Vec::new();
        let path = path.as_ref().to_path_buf();
        for c in read_dir(&path, "object dir")? {
            let c = dir_entry(c, "object entry")?;
            if c.file_name() == "." || c.file_name() == ".." {
                continue;
            }
            let p = c.path();
            ret.push(load_object(push_path(&path, &p), load_opt, p.display())?);
        }
        Ok(ret)
    }
}
