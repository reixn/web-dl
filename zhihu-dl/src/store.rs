use crate::{
    item::{self},
    meta::Version,
};
use serde::Serialize;
use std::{
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

pub(crate) mod info {
    use crate::{
        element::author::UserId,
        item::{AnswerId, ArticleId, CollectionId, ColumnId, CommentId, PinId, QuestionId},
    };
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;

    #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
    pub struct ItemInfo {
        pub in_store: bool,
        pub on_server: bool,
    }
    impl Default for ItemInfo {
        fn default() -> Self {
            Self {
                in_store: false,
                on_server: true,
            }
        }
    }
    #[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
    pub struct Answer {
        pub container: ItemInfo,
        pub comment: bool,
    }
    #[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
    pub struct Article {
        pub container: ItemInfo,
        pub comment: bool,
    }
    #[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
    pub struct Collection {
        pub container: ItemInfo,
        pub item: bool,
        pub comment: bool,
    }
    #[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
    pub struct Column {
        pub container: ItemInfo,
        pub item: bool,
        pub pinned_item: bool,
    }
    #[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
    pub struct Comment {
        pub container: ItemInfo,
        pub comment: bool,
    }
    #[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
    pub struct Pin {
        pub container: ItemInfo,
        pub comment: bool,
    }
    #[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
    pub struct Question {
        pub container: ItemInfo,
        pub answer: bool,
        pub comment: bool,
    }
    #[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
    pub struct UserCollection {
        pub created: bool,
        pub liked: bool,
    }
    #[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
    pub struct User {
        pub container: ItemInfo,
        pub activity: bool,
        pub answer: bool,
        pub article: bool,
        pub collection: UserCollection,
        pub column: bool,
        pub pin: bool,
        pub question: bool,
    }
    #[derive(Debug, Default, Serialize, Deserialize)]
    pub struct Info {
        pub answer: BTreeMap<AnswerId, Answer>,
        pub article: BTreeMap<ArticleId, Article>,
        pub collection: BTreeMap<CollectionId, Collection>,
        pub column: BTreeMap<ColumnId, Column>,
        pub comment: BTreeMap<CommentId, Comment>,
        pub pin: BTreeMap<PinId, Pin>,
        pub question: BTreeMap<QuestionId, Question>,
        pub user: BTreeMap<UserId, User>,
    }
}
pub use info::Info as ObjectInfo;

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
    path
}
pub trait BasicStoreItem: HasId + storable::Storable + media::StoreImage {
    fn in_store(id: Self::Id<'_>, store: &ObjectInfo) -> info::ItemInfo;
    fn add_info(id: Self::Id<'_>, info: info::ItemInfo, store: &mut ObjectInfo);
}
macro_rules! basic_store_item {
    ($t:ty, $i:ident) => {
        impl BasicStoreItem for $t {
            fn in_store(
                id: Self::Id<'_>,
                store: &crate::store::ObjectInfo,
            ) -> crate::store::info::ItemInfo {
                store.$i.get(&id).copied().unwrap_or_default().container
            }
            fn add_info(
                id: Self::Id<'_>,
                info: crate::store::info::ItemInfo,
                store: &mut crate::store::ObjectInfo,
            ) {
                store.$i.entry(id).or_default().container = info;
            }
        }
    };
}

pub const VERSION: Version = Version { major: 1, minor: 1 };
pub struct Store {
    version: Version,
    dirty: bool,
    root: PathBuf,
    pub(crate) objects: ObjectInfo,
}
const WEBSITE: &str = "zhihu.com";
const OBJECT_INFO: &str = "objects.yaml";
const VERSION_FILE: &str = "version.yaml";

#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    #[error("failed to open store")]
    OpenStore(#[source] StoreError),
    #[error("unexpected store version {0}, expect {}", Version{major:1,minor:0})]
    Version(Version),
    #[error("failed to load object {kind} {id}")]
    LoadObject {
        kind: &'static str,
        id: String,
        #[source]
        source: storable::Error,
    },
    #[error("failed to migrate image of {kind} {id}")]
    Image {
        kind: &'static str,
        id: String,
        #[source]
        source: media::Error,
    },
    #[error("failed to save store state")]
    SaveStore(#[source] StoreError),
}
impl Store {
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self, StoreError> {
        let root = {
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
            path.join(WEBSITE)
        };
        fs::create_dir(&root).map_err(|e| StoreError::Fs {
            op: FsErrorOp::CreateDir,
            path: root.clone(),
            source: e,
        })?;
        Ok(Self {
            version: {
                store_yaml(&VERSION, &root, VERSION_FILE)?;
                VERSION
            },
            dirty: false,
            objects: {
                let ret = ObjectInfo::default();
                store_yaml(&ret, root.as_path(), OBJECT_INFO)?;
                ret
            },
            root,
        })
    }
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StoreError> {
        let root = {
            let path = path.as_ref().canonicalize().map_err(|e| StoreError::Fs {
                op: FsErrorOp::CanonicalizePath,
                path: path.as_ref().to_path_buf(),
                source: e,
            })?;
            path.join(WEBSITE)
        };
        let version = load_yaml(&root, || Version { major: 0, minor: 0 }, VERSION_FILE)?;
        if !VERSION.is_compatible(version) {
            return Err(StoreError::Version(version));
        }
        Ok(Self {
            version,
            objects: load_yaml(&root, ObjectInfo::default, OBJECT_INFO)?,
            dirty: false,
            root,
        })
    }

    fn migrate_item<I: HasId + BasicStoreItem + media::StoreImage>(
        &self,
        image_store: &PathBuf,
        id: I::Id<'_>,
    ) -> Result<(), MigrateError> {
        let sp = self.store_path::<I>(id);
        let item = I::load(&sp, Default::default()).map_err(|e| MigrateError::LoadObject {
            kind: I::TYPE,
            id: id.to_string(),
            source: e,
        })?;
        item.migrate(image_store, sp)
            .map_err(|e| MigrateError::Image {
                kind: I::TYPE,
                id: id.to_string(),
                source: e,
            })
    }
    pub fn migrate<P: AsRef<Path>>(path: P) -> Result<(), MigrateError> {
        let mut store = Self::open(path).map_err(MigrateError::OpenStore)?;
        if store.version != (Version { major: 1, minor: 0 }) {
            return Err(MigrateError::Version(store.version));
        }
        let image_store = store.root.with_file_name("images");
        for (id, info) in &store.objects.answer {
            if info.container.in_store {
                store.migrate_item::<item::Answer>(&image_store, *id)?;
            }
        }
        for (id, info) in &store.objects.article {
            if info.container.in_store {
                store.migrate_item::<item::Article>(&image_store, *id)?;
            }
        }
        for (id, info) in &store.objects.collection {
            if info.container.in_store {
                store.migrate_item::<item::Collection>(&image_store, *id)?;
            }
        }
        for (id, info) in &store.objects.column {
            if info.container.in_store {
                store.migrate_item::<item::Column>(
                    &image_store,
                    item::column::ColumnRef(id.0.as_str()),
                )?;
            }
        }
        for (id, info) in &store.objects.comment {
            if info.container.in_store {
                store.migrate_item::<item::Comment>(&image_store, *id)?;
            }
        }
        for (id, info) in &store.objects.pin {
            if info.container.in_store {
                store.migrate_item::<item::Pin>(&image_store, *id)?;
            }
        }
        for (id, info) in &store.objects.question {
            if info.container.in_store {
                store.migrate_item::<item::Question>(&image_store, *id)?;
            }
        }
        for (id, info) in &store.objects.user {
            if info.container.in_store {
                store.migrate_item::<item::User>(&image_store, item::user::StoreId(*id, ""))?;
            }
        }
        store.version = VERSION;
        store.save().map_err(MigrateError::SaveStore)
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn root(&self) -> &PathBuf {
        &self.root
    }
    pub fn save(&mut self) -> Result<(), StoreError> {
        store_yaml(&self.version, &self.root, VERSION_FILE)?;
        store_yaml(&self.objects, &self.root, OBJECT_INFO)?;
        self.dirty = false;
        Ok(())
    }

    pub fn store_path<I: HasId>(&self, id: I::Id<'_>) -> PathBuf {
        let mut ret = self.item_path::<I>(id);
        ret.push("info");
        ret
    }
    pub fn item_path<I: HasId>(&self, id: I::Id<'_>) -> PathBuf {
        item_path::<I, _>(id, &self.root)
    }
    pub fn container_store_path<IC: BasicStoreContainer<O, I>, O, I: HasId + 'static>(
        &self,
        id: IC::Id<'_>,
    ) -> PathBuf {
        let mut ret = self.item_path::<IC>(id);
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
    pub fn get_media<I: HasId + media::StoreImage>(
        &mut self,
        object: &mut I,
    ) -> Result<(), media::Error> {
        object.load_images(self.store_path::<I>(object.id()))
    }
    pub fn get_container<O, I: HasId, IC: BasicStoreContainer<O, I>>(
        &self,
        id: IC::Id<'_>,
    ) -> Result<IC::ItemList, StoreError> {
        load_yaml(
            self.container_store_path::<IC, O, I>(id),
            IC::ItemList::default,
            ITEM_LIST,
        )
    }

    pub fn add_media<I: BasicStoreItem + media::StoreImage>(
        &mut self,
        data: &I,
    ) -> Result<(), media::Error> {
        data.store_images(self.store_path::<I>(data.id()))
    }
    pub fn add_object<I: BasicStoreItem>(
        &mut self,
        on_server: bool,
        object: &I,
    ) -> Result<PathBuf, storable::Error> {
        let path = self.store_path::<I>(object.id());
        object.store(&path)?;
        <I as StoreItem>::add_info(
            object.id(),
            info::ItemInfo {
                in_store: true,
                on_server,
            },
            self,
        );
        Ok(path)
    }
    pub fn add_container<'a, 'b, IC: BasicStoreContainer<O, I>, O, I: HasId>(
        &'a mut self,
        id: IC::Id<'b>,
    ) -> Result<Container<'b, 'a, IC, O, I>, StoreError> {
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
            absent_list: item_list.clone(),
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
    fn in_store(id: Self::Id<'_>, store: &Store) -> info::ItemInfo;
    fn add_info(id: Self::Id<'_>, info: info::ItemInfo, store: &mut Store);
    fn link_info<P: AsRef<Path>>(id: Self::Id<'_>, store: &Store, dest: P) -> Option<LinkInfo>;
    fn add_media(&self, store: &mut Store) -> Result<(), media::Error>;
    fn save_data(
        &self,
        on_server: bool,
        store: &mut Store,
    ) -> Result<Option<PathBuf>, storable::Error>;
    fn save_data_link<P: AsRef<Path>>(
        &self,
        on_server: bool,
        store: &mut Store,
        dest: P,
    ) -> Result<Option<LinkInfo>, storable::Error>;
}

impl<I: BasicStoreItem> StoreItem for I {
    fn in_store(id: Self::Id<'_>, store: &Store) -> info::ItemInfo {
        <Self as BasicStoreItem>::in_store(id, &store.objects)
    }
    fn link_info<P: AsRef<Path>>(id: Self::Id<'_>, store: &Store, dest: P) -> Option<LinkInfo> {
        Some(LinkInfo {
            source: store.item_path::<Self>(id),
            link: item_path::<Self, _>(id, dest),
        })
    }
    fn add_info(id: Self::Id<'_>, info: info::ItemInfo, store: &mut Store) {
        <Self as BasicStoreItem>::add_info(id, info, &mut store.objects);
        store.dirty = true;
    }
    fn add_media(&self, store: &mut Store) -> Result<(), media::Error> {
        store.add_media(self)
    }
    fn save_data(
        &self,
        on_server: bool,
        store: &mut Store,
    ) -> Result<Option<PathBuf>, storable::Error> {
        Ok(Some(store.add_object(on_server, self)?))
    }
    fn save_data_link<P: AsRef<Path>>(
        &self,
        on_server: bool,
        store: &mut Store,
        dest: P,
    ) -> Result<Option<LinkInfo>, storable::Error> {
        let source = store.add_object(on_server, self)?;
        Ok(Some(LinkInfo {
            source,
            link: item_path::<Self, _>(self.id(), dest),
        }))
    }
}

pub trait ItemList<I: HasId + 'static>:
    Default + Clone + Serialize + serde::de::DeserializeOwned
{
    fn insert(&mut self, id: I::Id<'_>);
    fn remove(&mut self, id: I::Id<'_>);
    fn set_item_info(&self, info: info::ItemInfo, store: &mut Store);
}
macro_rules! item_list_btree {
    ($t:ty, $i:ty) => {
        impl crate::store::ItemList<$t> for std::collections::BTreeSet<$i> {
            fn insert(&mut self, id: $i) {
                self.insert(id);
            }
            fn remove(&mut self, id: $i) {
                self.remove(&id);
            }
            fn set_item_info(
                &self,
                info: crate::store::info::ItemInfo,
                store: &mut crate::store::Store,
            ) {
                for i in self.iter() {
                    <$t as crate::store::StoreItem>::add_info(*i, info, store);
                }
            }
        }
    };
}

pub trait BasicStoreContainer<O, I: HasId + 'static>: HasId {
    const OPTION_NAME: &'static str;
    type ItemList: ItemList<I>;
    fn in_store(id: Self::Id<'_>, store: &Store) -> bool;
    fn add_info(id: Self::Id<'_>, store: &mut Store);
}

pub trait ContainerHandle<I: HasId> {
    fn link_item(&mut self, id: I::Id<'_>) -> Result<(), StoreError>;
    fn mark_missing(&mut self);
    fn finish(self) -> Result<Option<PathBuf>, StoreError>;
}
pub trait StoreContainer<O, I: HasId>: HasId + 'static {
    const OPTION_NAME: &'static str;
    fn in_store(id: Self::Id<'_>, store: &Store) -> bool;
    fn store_path(id: Self::Id<'_>, store: &Store) -> Option<PathBuf>;
    type Handle<'a, 'b>: ContainerHandle<I>;
    fn save_data<'a, 'b>(
        id: Self::Id<'a>,
        store: &'b mut Store,
    ) -> Result<Self::Handle<'a, 'b>, StoreError>;
}

const ITEM_LIST: &str = "item_list.yaml";
pub struct Container<'a, 'b, IC: 'a + BasicStoreContainer<O, I>, O, I: HasId + 'static> {
    store: &'b mut Store,
    root: PathBuf,
    id: IC::Id<'a>,
    item_list: IC::ItemList,
    absent_list: IC::ItemList,
    _o: PhantomData<O>,
    _i: PhantomData<I>,
}
impl<'a, 'b, IC: 'a + BasicStoreContainer<O, I>, O, I: HasId + 'static>
    Container<'a, 'b, IC, O, I>
{
    pub(crate) fn finish_container(self) -> Result<PathBuf, StoreError> {
        IC::add_info(self.id, self.store);
        store_yaml(&self.item_list, &self.root, ITEM_LIST)?;
        Ok(self.root)
    }
}
impl<'a, 'b, IC: 'a + BasicStoreContainer<O, I>, O, I: StoreItem + 'static> ContainerHandle<I>
    for Container<'a, 'b, IC, O, I>
{
    fn link_item(&mut self, id: I::Id<'_>) -> Result<(), StoreError> {
        self.item_list.insert(id);
        self.absent_list.remove(id);
        if let Some(v) = I::link_info(id, self.store, &self.root) {
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
    fn mark_missing(&mut self) {
        self.absent_list.set_item_info(
            info::ItemInfo {
                in_store: true,
                on_server: false,
            },
            self.store,
        );
    }
    fn finish(self) -> Result<Option<PathBuf>, StoreError> {
        self.finish_container().map(Some)
    }
}
impl<I, O, IC> StoreContainer<O, I> for IC
where
    I: HasId + StoreItem + 'static,
    IC: BasicStoreContainer<O, I> + 'static,
{
    const OPTION_NAME: &'static str = <IC as BasicStoreContainer<O, I>>::OPTION_NAME;
    fn in_store(id: Self::Id<'_>, store: &Store) -> bool {
        <Self as BasicStoreContainer<O, I>>::in_store(id, store)
    }
    fn store_path(id: Self::Id<'_>, store: &Store) -> Option<PathBuf> {
        Some(store.container_store_path::<IC, O, I>(id))
    }
    type Handle<'a, 'b> = Container<'a, 'b, IC, O, I>;
    fn save_data<'a, 'b>(
        id: Self::Id<'a>,
        store: &'b mut Store,
    ) -> Result<Self::Handle<'a, 'b>, StoreError> {
        store.add_container::<IC, O, I>(id)
    }
}
