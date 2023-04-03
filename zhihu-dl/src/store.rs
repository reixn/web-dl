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
impl StoreObject {
    fn save(&self, path: PathBuf) -> Result<(), StoreError> {
        serde_yaml::to_writer(
            io::BufWriter::new({
                fs::File::create(path.as_path()).map_err(|e| StoreError::Fs {
                    op: FsErrorOp::CreateFile,
                    path,
                    source: e,
                })?
            }),
            &self,
        )
        .map_err(StoreError::from)
    }
}

fn item_path<I: HasId>(id: I::Id<'_>, mut path: PathBuf) -> PathBuf {
    path.push(I::TYPE);
    path.push(id.to_string());
    path
}
pub trait BasicStoreItem: HasId + storable::Storable {
    fn in_store(id: Self::Id<'_>, info: &StoreObject) -> bool;
    fn add_info(&self, info: &mut StoreObject);
}

pub struct Store {
    dirty: bool,
    root: PathBuf,
    objects: StoreObject,
    media_storer: media::Storer,
    media_loader: media::Loader,
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
        let path = path.as_ref().canonicalize().map_err(|e| StoreError::Fs {
            op: FsErrorOp::CanonicalizePath,
            path: path.as_ref().to_path_buf(),
            source: e,
        })?;
        let image_dir = path.join(IMAGE_DIR);
        Ok(Self {
            dirty: false,
            objects: {
                let ret = StoreObject::default();
                ret.save(path.join(OBJECT_INFO))?;
                ret
            },
            media_storer: media::Storer::new({
                match fs::create_dir(&image_dir) {
                    Ok(_) => image_dir.as_path(),
                    Err(e) => {
                        return Err(StoreError::Fs {
                            op: FsErrorOp::CreateDir,
                            path: image_dir,
                            source: e,
                        })
                    }
                }
            }),
            media_loader: media::Loader::new(image_dir),
            root: path,
        })
    }
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StoreError> {
        let path = path.as_ref().canonicalize().map_err(|e| StoreError::Fs {
            op: FsErrorOp::CanonicalizePath,
            path: path.as_ref().to_path_buf(),
            source: e,
        })?;
        let media_dir = path.join(IMAGE_DIR);
        Ok(Self {
            objects: serde_yaml::from_reader(io::BufReader::new({
                let path = path.join(OBJECT_INFO);
                fs::File::open(&path).map_err(|e| StoreError::Fs {
                    op: FsErrorOp::OpenFile,
                    path,
                    source: e,
                })?
            }))
            .map_err(StoreError::from)?,
            dirty: false,
            media_storer: media::Storer::new(&media_dir),
            media_loader: media::Loader::new(media_dir),
            root: path,
        })
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn save(&mut self) -> Result<(), StoreError> {
        self.objects.save(self.root.join(OBJECT_INFO))?;
        self.dirty = false;
        Ok(())
    }
    pub fn store_path<I: HasId>(&self, id: I::Id<'_>) -> PathBuf {
        item_path::<I>(id, self.root.clone())
    }
    pub fn add_media<I: media::HasImage>(&mut self, data: &I) -> Result<(), media::Error> {
        data.store_images(&mut self.media_storer)
    }
    pub fn get_object<I: BasicStoreItem>(
        &mut self,
        id: I::Id<'_>,
        load_opt: storable::LoadOpt,
    ) -> Result<I, storable::Error> {
        let mut path = self.root.join(I::TYPE);
        path.push(id.to_string());
        I::load(path, load_opt)
    }
    pub fn get_media<I: media::HasImage>(&mut self, object: &mut I) -> Result<(), media::Error> {
        object.load_images(&mut self.media_loader)
    }
    pub fn add_object<I: BasicStoreItem>(
        &mut self,
        object: &I,
    ) -> Result<PathBuf, storable::Error> {
        let path = self.store_path::<I>(object.id());
        object.store(&path)?;
        object.add_info(&mut self.objects);
        self.dirty = true;
        Ok(path)
    }
}

pub struct LinkInfo {
    pub(crate) source: PathBuf,
    pub(crate) link: PathBuf,
}
pub trait StoreItem: HasId {
    fn in_store(id: Self::Id<'_>, store: &Store) -> bool;
    fn link_info(id: Self::Id<'_>, store: &Store, dest: Option<PathBuf>) -> Option<LinkInfo>;
    fn save_data(
        &self,
        store: &mut Store,
        dest: Option<PathBuf>,
    ) -> Result<Option<LinkInfo>, storable::Error>;
}

impl<I: BasicStoreItem> StoreItem for I {
    fn in_store(id: Self::Id<'_>, store: &Store) -> bool {
        <Self as BasicStoreItem>::in_store(id, &store.objects)
    }
    fn link_info(id: Self::Id<'_>, store: &Store, dest: Option<PathBuf>) -> Option<LinkInfo> {
        dest.map(|dest| LinkInfo {
            source: store.store_path::<Self>(id),
            link: item_path::<Self>(id, dest),
        })
    }
    fn save_data(
        &self,
        store: &mut Store,
        dest: Option<PathBuf>,
    ) -> Result<Option<LinkInfo>, storable::Error> {
        let source = store.add_object(self)?;
        Ok(dest.map(|dest| LinkInfo {
            source,
            link: item_path::<Self>(self.id(), dest),
        }))
    }
}
