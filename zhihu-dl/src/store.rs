use crate::{
    item::{
        answer::{Answer, AnswerId},
        article::{Article, ArticleId},
        collection::{Collection, CollectionId},
        column::{Column, ColumnId},
        pin::{Pin, PinId},
        question::{Question, QuestionId},
        user::{User, UserId},
    },
    meta::Version,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    fs, io,
    marker::PhantomData,
    path::{Path, PathBuf},
};
use thiserror::Error;
use web_dl_base::{id::HasId, media, storable};

#[derive(Debug, Clone)]
pub enum FsErrorOp {
    CreateDir,
    CreateFile,
    OpenFile,
    CanonicalizePath,
    SymLinkTo(PathBuf),
    RenameTo(PathBuf),
    OpenDir,
    GetDirEntry,
}
impl Display for FsErrorOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FsErrorOp::CreateDir => f.write_str("create directory"),
            FsErrorOp::CreateFile => f.write_str("create file"),
            FsErrorOp::OpenFile => f.write_str("open file"),
            FsErrorOp::CanonicalizePath => f.write_str("canonicalize path"),
            FsErrorOp::SymLinkTo(v) => write!(f, "create symbolic link to {} from", v.display()),
            FsErrorOp::RenameTo(t) => write!(f, "rename to {} from", t.display()),
            FsErrorOp::OpenDir => f.write_str("open directory"),
            FsErrorOp::GetDirEntry => f.write_str("get directory entry"),
        }
    }
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("incompatible version: program {}, file {0}", VERSION)]
    Version(Version),
    #[error("io error: failed to {op} {}", path.display())]
    Fs {
        op: FsErrorOp,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("yaml error: failed to process {file}")]
    Yaml {
        file: &'static str,
        #[source]
        source: serde_yaml::Error,
    },
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ObjectInfo {
    pub(crate) answer: BTreeSet<AnswerId>,
    pub(crate) article: BTreeSet<ArticleId>,
    pub(crate) collection: BTreeSet<CollectionId>,
    pub(crate) column: BTreeSet<ColumnId>,
    pub(crate) pin: BTreeSet<PinId>,
    pub(crate) question: BTreeSet<QuestionId>,
    pub(crate) user: BTreeSet<UserId>,
}

pub(crate) mod container {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Default, Serialize, Deserialize)]
    pub struct Column {
        pub item: bool,
        pub pinned_item: bool,
    }

    #[derive(Debug, Default, Serialize, Deserialize)]
    pub struct UserCollection {
        pub created: bool,
        pub liked: bool,
    }
    #[derive(Debug, Default, Serialize, Deserialize)]
    pub struct User {
        pub activity: bool,
        pub answer: bool,
        pub article: bool,
        pub collection: UserCollection,
        pub column: bool,
        pub pin: bool,
        pub question: bool,
    }
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub(crate) collection: BTreeSet<CollectionId>,
    pub(crate) column: BTreeMap<ColumnId, container::Column>,
    pub(crate) question: BTreeSet<QuestionId>,
    pub(crate) user: BTreeMap<UserId, container::User>,
}

fn load_yaml<V: serde::de::DeserializeOwned, F: FnOnce() -> V, P: AsRef<Path>>(
    path: P,
    default: F,
    file: &'static str,
) -> Result<V, StoreError> {
    let path = path.as_ref().join(file);
    if !path.exists() {
        return Ok(default());
    }
    serde_yaml::from_reader(io::BufReader::new(fs::File::open(path.as_path()).map_err(
        |e| StoreError::Fs {
            op: FsErrorOp::OpenFile,
            path,
            source: e,
        },
    )?))
    .map_err(|e| StoreError::Yaml { file, source: e })
}
fn store_yaml<V: Serialize, P: AsRef<Path>>(
    value: &V,
    path: P,
    file: &'static str,
) -> Result<(), StoreError> {
    let path = path.as_ref().join(file);
    serde_yaml::to_writer(
        io::BufWriter::new(
            fs::File::create(path.as_path()).map_err(|e| StoreError::Fs {
                op: FsErrorOp::CreateFile,
                path,
                source: e,
            })?,
        ),
        value,
    )
    .map_err(|e| StoreError::Yaml { file, source: e })
}

fn item_path<I: HasId, P: AsRef<Path>>(id: I::Id<'_>, path: P) -> PathBuf {
    let mut path = path.as_ref().join(I::TYPE);
    path.push(id.to_string());
    path.push("info");
    path
}
pub trait BasicStoreItem: HasId + storable::Storable {
    fn in_store(id: Self::Id<'_>, info: &ObjectInfo) -> bool;
    fn add_info(&self, info: &mut ObjectInfo);
}

const ITEM_LIST: &str = "item_list.yaml";
pub struct Container<'a, 'b, IC: 'b + StoreItemContainer<O, I>, O, I: HasId> {
    store: &'a mut Store,
    root: PathBuf,
    id: IC::Id<'b>,
    item_list: IC::ItemList,
    _o: PhantomData<O>,
    _i: PhantomData<I>,
}
impl<'a, 'b, IC: 'b + StoreItemContainer<O, I>, O, I: StoreItem> Container<'a, 'b, IC, O, I> {
    pub(crate) fn link_item(&mut self, id: I::Id<'_>) -> Result<(), StoreError> {
        IC::add_item(id, &mut self.item_list);
        if let Some(v) = I::link_info(id, &self.store, &self.root) {
            if v.link.exists() {
                return Ok(());
            }
            {
                let parent = v.link.parent().unwrap();
                if !parent.exists() {
                    fs::create_dir_all(parent).map_err(|e| StoreError::Fs {
                        op: FsErrorOp::CreateDir,
                        path: parent.to_path_buf(),
                        source: e,
                    })?;
                }
            }
            let mut source = PathBuf::new();
            for _ in v
                .link
                .strip_prefix(&self.store.root)
                .unwrap()
                .parent()
                .unwrap()
                .components()
            {
                source.push("..");
            }
            source.extend(v.source.strip_prefix(&self.store.root).unwrap());
            crate::util::relative_path::symlink(&source, &v.link).map_err(|e| StoreError::Fs {
                op: FsErrorOp::SymLinkTo(v.source),
                path: v.link,
                source: e,
            })
        } else {
            Ok(())
        }
    }
    pub(crate) fn finish(self) -> Result<PathBuf, StoreError> {
        IC::add_info(self.id, &mut self.store.containers);
        store_yaml(&self.item_list, &self.root, ITEM_LIST)?;
        Ok(self.root)
    }
}

pub const VERSION: Version = Version { major: 1, minor: 0 };
pub struct Store {
    version: Version,
    dirty: bool,
    root: PathBuf,
    objects: ObjectInfo,
    pub(crate) containers: ContainerInfo,
    media_storer: media::Storer,
    media_loader: media::Loader,
}
const OBJECT_INFO: &str = "objects.yaml";
const CONTAINER_INFO: &str = "container.yaml";
const IMAGE_DIR: &str = "images";
const VERSION_FILE: &str = "version.yaml";
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
        let version = load_yaml(&path, || Version { major: 0, minor: 0 }, VERSION_FILE)?;
        if !VERSION.is_compatible(version) {
            return Err(StoreError::Version(version));
        }
        let image_dir = path.join(IMAGE_DIR);
        Ok(Self {
            version,
            dirty: false,
            objects: {
                let ret = ObjectInfo::default();
                store_yaml(&ret, path.as_path(), OBJECT_INFO)?;
                ret
            },
            containers: {
                let ret = ContainerInfo::default();
                store_yaml(&ret, path.as_path(), CONTAINER_INFO)?;
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
        let version = load_yaml(&path, || Version { major: 0, minor: 0 }, VERSION_FILE)?;
        if !VERSION.is_compatible(version) {
            return Err(StoreError::Version(version));
        }
        Ok(Self {
            version,
            objects: load_yaml(&path, ObjectInfo::default, OBJECT_INFO)?,
            containers: load_yaml(&path, ContainerInfo::default, CONTAINER_INFO)?,
            dirty: false,
            media_storer: media::Storer::new(&media_dir),
            media_loader: media::Loader::new(media_dir),
            root: path,
        })
    }
    pub fn migrate<P: AsRef<Path>>(path: P) -> Result<(), StoreError> {
        let path = path.as_ref();
        let version = load_yaml(path, || Version { major: 0, minor: 0 }, VERSION_FILE)?;
        if version >= VERSION {
            return Err(StoreError::Version(version));
        }
        const TYPE_LIST: [&'static str; 7] = [
            Answer::TYPE,
            Article::TYPE,
            Collection::TYPE,
            Column::TYPE,
            Pin::TYPE,
            Question::TYPE,
            User::TYPE,
        ];
        for i in TYPE_LIST {
            let path = path.join(i);
            if !path.exists() {
                continue;
            }
            for entry in path.read_dir().map_err(|e| StoreError::Fs {
                op: FsErrorOp::OpenDir,
                path: path.clone(),
                source: e,
            })? {
                let entry = entry.map_err(|e| StoreError::Fs {
                    op: FsErrorOp::GetDirEntry,
                    path: path.clone(),
                    source: e,
                })?;
                let mut path = entry.path();
                let tmp_path = path.with_file_name("info");
                fs::rename(&path, &tmp_path).map_err(|e| StoreError::Fs {
                    op: FsErrorOp::RenameTo(tmp_path.clone()),
                    path: path.clone(),
                    source: e,
                })?;
                fs::create_dir(&path).map_err(|e| StoreError::Fs {
                    op: FsErrorOp::CreateDir,
                    path: path.clone(),
                    source: e,
                })?;
                path.push("info");
                fs::rename(&tmp_path, &path).map_err(|e| StoreError::Fs {
                    op: FsErrorOp::RenameTo(path),
                    path: tmp_path,
                    source: e,
                })?;
            }
        }
        store_yaml(&VERSION, path, VERSION_FILE)
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn image_path(&self) -> PathBuf {
        self.root.join(IMAGE_DIR)
    }
    pub fn save(&mut self) -> Result<(), StoreError> {
        store_yaml(&self.version, &self.root, VERSION_FILE)?;
        store_yaml(&self.objects, &self.root, OBJECT_INFO)?;
        store_yaml(&self.containers, &self.root, CONTAINER_INFO)?;
        self.dirty = false;
        Ok(())
    }

    pub fn store_path<I: HasId>(&self, id: I::Id<'_>) -> PathBuf {
        item_path::<I, _>(id, &self.root)
    }
    pub fn container_store_path<IC: StoreItemContainer<O, I>, O, I: HasId>(
        &self,
        id: IC::Id<'_>,
    ) -> PathBuf {
        let mut ret = self.root.join(IC::TYPE);
        ret.push(id.to_string());
        ret.push(IC::OPTION_NAME);
        ret
    }

    pub fn get_object<I: BasicStoreItem>(
        &mut self,
        id: I::Id<'_>,
        load_opt: storable::LoadOpt,
    ) -> Result<I, storable::Error> {
        I::load(self.store_path::<I>(id), load_opt)
    }
    pub fn get_media<I: media::HasImage>(&mut self, object: &mut I) -> Result<(), media::Error> {
        object.load_images(&mut self.media_loader)
    }

    pub fn add_media<I: media::HasImage>(&mut self, data: &I) -> Result<(), media::Error> {
        data.store_images(&mut self.media_storer)
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
    pub fn add_container<'a, 'b, IC: StoreItemContainer<O, I>, O, I: HasId>(
        &'a mut self,
        id: IC::Id<'b>,
    ) -> Result<Container<'a, 'b, IC, O, I>, StoreError> {
        let path = self.container_store_path::<IC, O, I>(id);
        let item_list = if !path.exists() {
            fs::create_dir_all(&path).map_err(|e| StoreError::Fs {
                op: FsErrorOp::CreateDir,
                path: path.clone(),
                source: e,
            })?;
            IC::ItemList::default()
        } else {
            load_yaml(&path, IC::ItemList::default, ITEM_LIST)?
        };
        Ok(Container {
            store: self,
            root: path,
            id,
            item_list,
            _o: PhantomData,
            _i: PhantomData,
        })
    }
}

pub struct LinkInfo {
    pub(crate) source: PathBuf,
    pub(crate) link: PathBuf,
}
pub trait StoreItem: HasId {
    fn in_store(id: Self::Id<'_>, store: &Store) -> bool;
    fn link_info<P: AsRef<Path>>(id: Self::Id<'_>, store: &Store, dest: P) -> Option<LinkInfo>;
    fn save_data(&self, store: &mut Store) -> Result<Option<PathBuf>, storable::Error>;
    fn save_data_link<P: AsRef<Path>>(
        &self,
        store: &mut Store,
        dest: P,
    ) -> Result<Option<LinkInfo>, storable::Error>;
}

impl<I: BasicStoreItem> StoreItem for I {
    fn in_store(id: Self::Id<'_>, store: &Store) -> bool {
        <Self as BasicStoreItem>::in_store(id, &store.objects)
    }
    fn link_info<P: AsRef<Path>>(id: Self::Id<'_>, store: &Store, dest: P) -> Option<LinkInfo> {
        Some(LinkInfo {
            source: store.store_path::<Self>(id),
            link: item_path::<Self, _>(id, dest),
        })
    }
    fn save_data(&self, store: &mut Store) -> Result<Option<PathBuf>, storable::Error> {
        Ok(Some(store.add_object(self)?))
    }
    fn save_data_link<P: AsRef<Path>>(
        &self,
        store: &mut Store,
        dest: P,
    ) -> Result<Option<LinkInfo>, storable::Error> {
        let source = store.add_object(self)?;
        Ok(Some(LinkInfo {
            source,
            link: item_path::<Self, _>(self.id(), dest),
        }))
    }
}

pub trait StoreItemContainer<O, I: HasId>: HasId {
    const OPTION_NAME: &'static str;
    type ItemList: Default + Serialize + serde::de::DeserializeOwned;
    fn in_store(id: Self::Id<'_>, info: &ContainerInfo) -> bool;
    fn add_info(id: Self::Id<'_>, info: &mut ContainerInfo);
    fn add_item(id: I::Id<'_>, list: &mut Self::ItemList);
}
