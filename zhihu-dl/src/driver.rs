use crate::{
    element::comment,
    item::{Fetchable, Item, ItemContainer},
    progress,
    raw_data::{self, RawData, RawDataInfo},
    request::Client,
    store::{BasicStoreItem, LinkInfo, Store, StoreError, StoreItem},
};
use chrono::Utc;
use serde::Deserialize;
use std::{
    fmt::Display,
    fs, io,
    path::{Component, Path, PathBuf},
};
use thiserror;
use web_dl_base::{id::HasId, media, storable};

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("failed to create dir {}", dir.display())]
    CreateDir {
        dir: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("store path `{}` and destination `{}` has different prefix", store_path.display(), dest.display())]
    DifferentPrefix { store_path: PathBuf, dest: PathBuf },
    #[error("failed to create sym link from {} to {}", link.display(), link_source.display())]
    SymLink {
        link_source: PathBuf,
        link: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum DestPrepError {
    #[error("failed to create destination dir")]
    CreateDir(#[source] io::Error),
    #[error("failed to canonicalize dest path")]
    Canonicalize(#[source] io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ItemError {
    #[error("failed to prepare destination")]
    DestPrep(
        #[source]
        #[from]
        DestPrepError,
    ),
    #[error("http request error occurred")]
    Http(
        #[source]
        #[from]
        reqwest::Error,
    ),
    #[error("failed to parse api response")]
    Json(
        #[source]
        #[from]
        serde_json::Error,
    ),
    #[error("failed to fetch comment")]
    Comment(
        #[source]
        #[from]
        comment::FetchError,
    ),
    #[error("failed to store data")]
    Store(
        #[source]
        #[from]
        storable::Error,
    ),
    #[error("failed to store images")]
    Media(
        #[source]
        #[from]
        media::Error,
    ),
    #[error("failed to link {} to {}", store_path.display() ,dest.display())]
    Link {
        store_path: PathBuf,
        dest: PathBuf,
        #[source]
        source: LinkError,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    #[error("failed to prepare destination")]
    DestPrep(
        #[source]
        #[from]
        DestPrepError,
    ),
    #[error("http request error occurred")]
    Http(
        #[source]
        #[from]
        reqwest::Error,
    ),
    #[error("failed to parse json response")]
    Json(
        #[source]
        #[from]
        serde_json::Error,
    ),
    #[error("failed to process item {id}")]
    Item {
        id: String,
        #[source]
        source: ItemError,
    },
}

fn prepare_dest(dest: &Path) -> Result<PathBuf, DestPrepError> {
    if !dest.exists() {
        fs::create_dir_all(dest).map_err(DestPrepError::CreateDir)?;
    }
    dest.canonicalize().map_err(DestPrepError::Canonicalize)
}

fn link_to_dest(relative: bool, store_path: &Path, dest: &Path) -> Result<(), LinkError> {
    fn symlink<P1: AsRef<Path>, P2: AsRef<Path>>(source: P1, link: P2) -> Result<(), io::Error> {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(source, link)
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir(source, link)
        }
    }
    if store_path == dest {
        return Ok(());
    }
    let dest_parent = dest.parent().unwrap();
    if !dest_parent.exists() {
        fs::create_dir_all(dest_parent).map_err(|e| LinkError::CreateDir {
            dir: dest_parent.to_path_buf(),
            source: e,
        })?;
    }
    if !relative {
        return symlink(store_path, dest).map_err(|e| LinkError::SymLink {
            link_source: store_path.to_path_buf(),
            link: dest.to_path_buf(),
            source: e,
        });
    }
    let link_source = {
        let mut ret = PathBuf::new();
        let mut store_com = store_path.components().peekable();
        let mut dest_com = dest.parent().unwrap().components().peekable();
        while store_com.peek() == dest_com.peek() {
            store_com.next();
            if dest_com.next().is_none() {
                break;
            }
        }
        for v in dest_com {
            match v {
                Component::Prefix(_) => {
                    return Err(LinkError::DifferentPrefix {
                        store_path: store_path.to_path_buf(),
                        dest: dest.to_path_buf(),
                    })
                }
                Component::Normal(_) => ret.push(".."),
                _ => unreachable!(),
            }
        }
        for v in store_com {
            match v {
                Component::Normal(d) => ret.push(d),
                _ => unreachable!(),
            }
        }
        ret
    };
    symlink(&link_source, dest).map_err(|e| LinkError::SymLink {
        link_source,
        link: dest.to_path_buf(),
        source: e,
    })
}

#[derive(Debug)]
pub struct ContainerItem<I> {
    pub processed: bool,
    pub value: I,
}

pub struct Driver {
    pub client: Client,
    pub store: Store,
}
impl Driver {
    pub fn create<P: AsRef<Path>>(store_path: P) -> Result<Self, StoreError> {
        Ok(Self {
            client: Client::new(),
            store: Store::create(store_path)?,
        })
    }
    pub fn open<P: AsRef<Path>>(store_path: P) -> Result<Self, StoreError> {
        Ok(Self {
            client: Client::new(),
            store: Store::open(store_path)?,
        })
    }
    pub fn save(&self) -> Result<(), StoreError> {
        self.store.save()
    }
    pub async fn init(&self) -> Result<(), reqwest::Error> {
        self.client.init().await
    }

    pub async fn process_item<I: Item, P: progress::ItemProg>(
        &self,
        prog: &P,
        item: &mut I,
        get_comment: bool,
    ) -> Result<(), ItemError> {
        log::debug!("processing {} {}", I::TYPE, item.id());
        if item.get_images(&self.client, prog).await {
            prog.sleep(self.client.request_interval).await;
        }
        if get_comment {
            item.get_comments(&self.client, prog)
                .await
                .map_err(ItemError::from)?;
            prog.sleep(self.client.request_interval).await;
        }
        Ok(())
    }

    async fn update_item_impl<'a, I, P>(
        &mut self,
        prog: &P,
        id: <I as HasId>::Id<'a>,
        get_comment: bool,
    ) -> Result<(I, PathBuf), ItemError>
    where
        I: Fetchable + Item + media::HasImage + BasicStoreItem,
        P: progress::ItemProg,
    {
        let mut ret: I = {
            log::debug!("fetching raw data for {} {}", I::TYPE, id);
            let data = I::fetch(&self.client, id).await.map_err(ItemError::from)?;
            log::trace!("raw data {:#?}", data);
            I::from_reply(
                I::Reply::deserialize(&data).map_err(ItemError::from)?,
                RawData {
                    info: RawDataInfo {
                        fetch_time: Utc::now(),
                        container: raw_data::Container::None,
                    },
                    data,
                },
            )
        };
        self.process_item(prog, &mut ret, get_comment).await?;
        let dest = self.store.add_object(&ret).map_err(ItemError::from)?;
        self.store.add_media(&ret).map_err(ItemError::from)?;
        Ok((ret, dest))
    }

    pub async fn get_item<'a, I, P>(
        &mut self,
        prog: &P,
        id: <I as HasId>::Id<'a>,
        get_comment: bool,
    ) -> Result<Option<I>, ItemError>
    where
        I: Fetchable + Item + media::HasImage + BasicStoreItem,
        P: progress::ItemProg,
    {
        Ok(if <I as StoreItem>::in_store(id, &self.store) {
            None
        } else {
            Some(self.update_item_impl(prog, id, get_comment).await?.0)
        })
    }

    pub async fn download_item<'a, I, P, Pat>(
        &mut self,
        prog: &P,
        id: <I as HasId>::Id<'a>,
        get_comment: bool,
        relative: bool,
        parent: Pat,
        name: &str,
    ) -> Result<Option<I>, ItemError>
    where
        I: Fetchable + Item + media::HasImage + BasicStoreItem,
        P: progress::ItemProg,
        Pat: AsRef<Path>,
    {
        let canon_dest = {
            let mut dest = prepare_dest(parent.as_ref()).map_err(ItemError::DestPrep)?;
            dest.push(name);
            dest
        };
        let v = self.get_item(prog, id, get_comment).await?;
        let store_path = self.store.store_path::<I>(id);
        match link_to_dest(relative, store_path.as_path(), &canon_dest) {
            Ok(_) => Ok(v),
            Err(e) => Err(ItemError::Link {
                store_path,
                dest: parent.as_ref().join(name),
                source: e,
            }),
        }
    }
    pub async fn update_item<'a, I, P>(
        &mut self,
        prog: &P,
        id: <I as HasId>::Id<'a>,
        get_comment: bool,
    ) -> Result<I, ItemError>
    where
        I: Fetchable + Item + media::HasImage + BasicStoreItem,
        P: progress::ItemProg,
    {
        self.update_item_impl(prog, id, get_comment)
            .await
            .map(|r| r.0)
    }

    async fn get_container_impl<'a, IC, I, O, P>(
        &mut self,
        prog: &P,
        id: <IC as HasId>::Id<'a>,
        option: O,
        get_comment: bool,
        canon_dest: Option<PathBuf>,
    ) -> Result<(Vec<ContainerItem<I>>, Vec<(usize, LinkInfo)>), ContainerError>
    where
        I: Item + StoreItem + media::HasImage,
        O: Display + Copy,
        IC: ItemContainer<I, O>,
        P: progress::ItemContainerProg,
    {
        let dat = IC::fetch_items(&self.client, prog, id, option)
            .await
            .map_err(ContainerError::from)?;
        let mut ret = Vec::with_capacity(dat.len());
        let mut link_path = Vec::with_capacity(dat.len());
        {
            let mut p = prog.start_items(dat.len() as u64);
            for (idx, i) in dat.into_iter().enumerate() {
                use progress::ItemsProg;
                log::trace!("parsing response {:#?}", i);
                let mut item = IC::parse_item(i).map_err(ContainerError::from)?;
                if I::in_store(item.id(), &self.store) {
                    if let Some(li) = I::link_info(item.id(), &self.store, canon_dest.clone()) {
                        link_path.push((idx, li));
                    }
                    ret.push(ContainerItem {
                        processed: false,
                        value: item,
                    });
                    p.skip_item();
                    continue;
                }
                let i_p = p.start_item(I::TYPE, item.id());
                self.process_item(&i_p, &mut item, get_comment)
                    .await
                    .map_err(|e| ContainerError::Item {
                        id: item.id().to_string(),
                        source: e,
                    })?;
                if let Some(v) = item
                    .save_data(&mut self.store, canon_dest.clone())
                    .map_err(|e| ContainerError::Item {
                        id: item.id().to_string(),
                        source: ItemError::Store(e),
                    })?
                {
                    link_path.push((idx, v));
                }
                self.store
                    .add_media(&item)
                    .map_err(|e| ContainerError::Item {
                        id: item.id().to_string(),
                        source: ItemError::Media(e),
                    })?;
                ret.push(ContainerItem {
                    processed: true,
                    value: item,
                });
            }
        }
        Ok((ret, link_path))
    }

    pub async fn get_container<'a, IC, I, O, P>(
        &mut self,
        prog: &P,
        id: <IC as HasId>::Id<'a>,
        option: O,
        get_comment: bool,
    ) -> Result<Vec<ContainerItem<I>>, ContainerError>
    where
        I: Item + StoreItem + media::HasImage,
        O: Display + Copy,
        IC: ItemContainer<I, O>,
        P: progress::ItemContainerProg,
    {
        self.get_container_impl::<IC, I, O, P>(prog, id, option, get_comment, None)
            .await
            .map(|r| r.0)
    }
    pub async fn download_container<'a, IC, I, O, P, Pat>(
        &mut self,
        prog: &P,
        id: <IC as HasId>::Id<'a>,
        option: O,
        get_comment: bool,
        relative: bool,
        dest: Pat,
    ) -> Result<Vec<ContainerItem<I>>, ContainerError>
    where
        I: Item + StoreItem + media::HasImage,
        O: Display + Copy,
        IC: ItemContainer<I, O>,
        P: progress::ItemContainerProg,
        Pat: AsRef<Path>,
    {
        let canon_dest = prepare_dest(dest.as_ref()).map_err(ContainerError::from)?;
        let (ret, link_path) = self
            .get_container_impl::<IC, I, O, P>(prog, id, option, get_comment, Some(canon_dest))
            .await?;
        for (idx, li) in link_path {
            link_to_dest(relative, li.source.as_path(), li.link.as_path()).map_err(|e| {
                ContainerError::Item {
                    id: ret[idx].value.id().to_string(),
                    source: ItemError::Link {
                        store_path: li.source,
                        dest: li.link,
                        source: e,
                    },
                }
            })?;
        }
        Ok(ret)
    }
}
