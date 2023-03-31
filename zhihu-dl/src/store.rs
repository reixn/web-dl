use crate::item::{
    answer::AnswerId, article::ArticleId, collection::CollectionId, column::ColumnId, pin::PinId,
    question::QuestionId, user::UserId,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fmt::Display,
    fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;
use web_dl_base::{id::HasId, media, storable};

#[derive(Debug, Clone, Copy)]
pub enum FsErrorOp {
    CreateDir,
    CreateFile,
    OpenFile,
    CanonicalizePath,
}
impl Display for FsErrorOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FsErrorOp::CreateDir => f.write_str("create directory"),
            FsErrorOp::CreateFile => f.write_str("create file"),
            FsErrorOp::OpenFile => f.write_str("open file"),
            FsErrorOp::CanonicalizePath => f.write_str("canonicalize path"),
        }
    }
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("io error: failed to {op} {}", path.display())]
    Fs {
        op: FsErrorOp,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("yaml error")]
    Yaml(
        #[source]
        #[from]
        serde_yaml::Error,
    ),
}

#[derive(Default, Serialize, Deserialize)]
pub struct StoreObject {
    pub(crate) answer: BTreeSet<AnswerId>,
    pub(crate) article: BTreeSet<ArticleId>,
    pub(crate) collection: BTreeSet<CollectionId>,
    pub(crate) column: BTreeSet<ColumnId>,
    pub(crate) pin: BTreeSet<PinId>,
    pub(crate) question: BTreeSet<QuestionId>,
    pub(crate) user: BTreeSet<UserId>,
}

pub(crate) fn item_path<I: HasId>(id: I::Id<'_>, mut path: PathBuf) -> PathBuf {
    path.push(I::TYPE);
    path.push(id.to_string());
    path
}
pub trait BasicStoreItem: HasId + storable::Storable {
    fn in_store(id: Self::Id<'_>, info: &StoreObject) -> bool;
    fn add_info(&self, info: &mut StoreObject);
}

pub struct Store {
    root: PathBuf,
    objects: StoreObject,
    media_storer: media::Storer,
}

const OBJECT_INFO: &str = "objects.yaml";
const IMAGE_DIR: &str = "images";
impl Store {
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self, StoreError> {
        fs::create_dir_all(path.as_ref()).map_err(|e| StoreError::Fs {
            op: FsErrorOp::CreateDir,
            path: path.as_ref().to_path_buf(),
            source: e,
        })?;
        let mut path = path.as_ref().canonicalize().map_err(|e| StoreError::Fs {
            op: FsErrorOp::CanonicalizePath,
            path: path.as_ref().to_path_buf(),
            source: e,
        })?;
        Ok(Self {
            root: path.clone(),
            objects: StoreObject::default(),
            media_storer: media::Storer::new({
                path.push(IMAGE_DIR);
                fs::create_dir(&path).map_err(|e| StoreError::Fs {
                    op: FsErrorOp::CreateDir,
                    path: path.clone(),
                    source: e,
                })?;
                path
            }),
        })
    }
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StoreError> {
        let mut path = path.as_ref().canonicalize().map_err(|e| StoreError::Fs {
            op: FsErrorOp::CanonicalizePath,
            path: path.as_ref().to_path_buf(),
            source: e,
        })?;
        Ok(Self {
            root: path.clone(),
            objects: serde_yaml::from_reader(io::BufReader::new({
                let mut path = path.clone();
                path.push(OBJECT_INFO);
                fs::File::open(&path).map_err(|e| StoreError::Fs {
                    op: FsErrorOp::OpenFile,
                    path: path.clone(),
                    source: e,
                })?
            }))
            .map_err(StoreError::from)?,
            media_storer: {
                path.push(IMAGE_DIR);
                media::Storer::new(path)
            },
        })
    }
    pub fn save(&self) -> Result<(), StoreError> {
        serde_yaml::to_writer(
            io::BufWriter::new({
                let mut path = self.root.clone();
                path.push(OBJECT_INFO);
                fs::File::create(&path).map_err(|e| StoreError::Fs {
                    op: FsErrorOp::CreateFile,
                    path: path.clone(),
                    source: e,
                })?
            }),
            &self.objects,
        )
        .map_err(StoreError::from)
    }
    pub fn store_path<I: HasId>(&self, id: I::Id<'_>) -> PathBuf {
        item_path::<I>(id, self.root.clone())
    }
    pub(crate) fn add_media<I: media::HasImage>(&mut self, data: &I) -> Result<(), media::Error> {
        data.store_images(&mut self.media_storer)
    }
    pub(crate) fn add_object<I: BasicStoreItem>(
        &mut self,
        object: &I,
    ) -> Result<PathBuf, storable::Error> {
        let path = self.store_path::<I>(object.id());
        object.store(&path)?;
        object.add_info(&mut self.objects);
        Ok(path)
    }
}

pub struct LinkInfo {
    pub source: PathBuf,
    pub link: PathBuf,
}
pub trait StoreItem: HasId {
    fn in_store(id: Self::Id<'_>, store: &Store) -> bool;
    fn link_info(id: Self::Id<'_>, store: &Store, dest: PathBuf) -> Option<LinkInfo>;
    fn save_data(
        &self,
        store: &mut Store,
        dest: PathBuf,
    ) -> Result<Option<LinkInfo>, storable::Error>;
}

impl<I: BasicStoreItem> StoreItem for I {
    fn in_store(id: Self::Id<'_>, store: &Store) -> bool {
        <Self as BasicStoreItem>::in_store(id, &store.objects)
    }
    fn link_info(id: Self::Id<'_>, store: &Store, dest: PathBuf) -> Option<LinkInfo> {
        Some(LinkInfo {
            source: store.store_path::<Self>(id),
            link: item_path::<Self>(id, dest),
        })
    }
    fn save_data(
        &self,
        store: &mut Store,
        dest: PathBuf,
    ) -> Result<Option<LinkInfo>, storable::Error> {
        store.add_object(self).map(|v| {
            Some(LinkInfo {
                source: v,
                link: item_path::<Self>(self.id(), dest),
            })
        })
    }
}