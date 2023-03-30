use super::hash::HashDigest;
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    error,
    fmt::Display,
    fs, io,
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(Debug)]
pub enum ErrorOp {
    Read(PathBuf),
    Write(PathBuf),
    HardLink { original: PathBuf, link: PathBuf },
}
#[derive(Debug)]
pub struct Error {
    op: ErrorOp,
    source: io::Error,
}
impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.op {
            ErrorOp::Read(b) => f.write_fmt(format_args!("failed to read `{}`", b.display())),
            ErrorOp::Write(b) => f.write_fmt(format_args!("failed to write `{}`", b.display())),
            ErrorOp::HardLink { original, link } => f.write_fmt(format_args!(
                "failed to link `{}` to `{}`",
                original.display(),
                link.display()
            )),
        }
    }
}
impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.source)
    }
}

pub struct Loader {
    media_root: PathBuf,
    load_cache: HashMap<HashDigest, Rc<Vec<u8>>>,
}
impl Loader {
    pub fn new<P: AsRef<Path>>(media_root: P) -> Self {
        Self {
            media_root: media_root.as_ref().to_path_buf(),
            load_cache: HashMap::new(),
        }
    }
    pub(crate) fn load(
        &mut self,
        hash: &HashDigest,
        extension: &str,
    ) -> Result<Rc<Vec<u8>>, Error> {
        match self.load_cache.entry(hash.clone()) {
            Entry::Occupied(v) => Ok(Rc::clone(v.get())),
            Entry::Vacant(v) => {
                let p = hash.store_path(&self.media_root, extension);
                let data = Rc::new(fs::read(&p).map_err(|e| Error {
                    op: ErrorOp::Read(p.clone()),
                    source: e,
                })?);
                v.insert(Rc::clone(&data));
                Ok(data)
            }
        }
    }
}

pub struct Storer {
    media_root: PathBuf,
    store_cache: HashMap<HashDigest, (PathBuf, HashSet<String>)>,
}
impl Storer {
    pub fn new<P: AsRef<Path>>(media_root: P) -> Self {
        Self {
            media_root: media_root.as_ref().to_path_buf(),
            store_cache: HashMap::new(),
        }
    }
    pub(crate) fn store(
        &mut self,
        hash: &HashDigest,
        extension: &str,
        data: &Vec<u8>,
    ) -> Result<(), Error> {
        match self.store_cache.entry(hash.clone()) {
            Entry::Occupied(mut v) => {
                let (pb, m) = v.get_mut();
                if m.contains(extension) {
                    Ok(())
                } else {
                    let p = hash.store_path(&self.media_root, extension);
                    if !p.exists() {
                        fs::hard_link(&pb, &p).map_err(|e| Error {
                            op: ErrorOp::HardLink {
                                original: pb.to_path_buf(),
                                link: p,
                            },
                            source: e,
                        })?;
                    }
                    m.insert(extension.to_owned());
                    Ok(())
                }
            }
            Entry::Vacant(v) => {
                let p = hash.store_path(&self.media_root, extension);
                let pb = hash.store_path(&self.media_root, "");
                if !p.exists() {
                    if !pb.exists() {
                        fs::write(&pb, data).map_err(|e| Error {
                            op: ErrorOp::Write(pb.clone()),
                            source: e,
                        })?;
                    }
                    fs::hard_link(&pb, &p).map_err(|e| Error {
                        op: ErrorOp::HardLink {
                            original: pb.clone(),
                            link: p,
                        },
                        source: e,
                    })?;
                }
                v.insert((pb, HashSet::from([String::new(), extension.to_owned()])));
                Ok(())
            }
        }
    }
    pub fn refer_set(self) -> Vec<PathBuf> {
        let mut ret = Vec::with_capacity(self.store_cache.iter().map(|(_, (_, s))| s.len()).sum());
        for (h, (_, es)) in self.store_cache.into_iter() {
            for e in es.into_iter() {
                ret.push(h.store_path(&self.media_root, e.as_str()))
            }
        }
        ret
    }
}

pub struct RefSet<'a> {
    references: HashSet<(&'a HashDigest, &'a str)>,
}
impl<'a> RefSet<'a> {
    pub fn new() -> Self {
        Self {
            references: HashSet::new(),
        }
    }
    pub(crate) fn add_root<'b>(&'b mut self, digest: &'a HashDigest, extension: &'a str)
    where
        'a: 'b,
    {
        self.references.insert((digest, extension));
    }
    pub fn refer_paths(self, media_root: PathBuf) -> Vec<PathBuf> {
        self.references
            .into_iter()
            .map(|(h, e)| h.store_path(&media_root, e))
            .collect()
    }
}
