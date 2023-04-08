use crate::{
    element::comment,
    item::{Fetchable, Item, ItemContainer},
    progress::{self, ContainerJob, ItemJob},
    raw_data::{self, RawData, RawDataInfo},
    request::Client,
    store::{BasicStoreItem, Store, StoreError, StoreItem},
    util::relative_path::{link_to_dest, prepare_dest, DestPrepError, LinkError},
};
use chrono::Utc;
use serde::Deserialize;
use std::{
    fmt::Display,
    path::{Path, PathBuf},
};
use thiserror;
use web_dl_base::{id::HasId, media, storable};

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
    #[error("failed to store container")]
    Store(
        #[source]
        #[from]
        StoreError,
    ),
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
    #[error("failed to link item {id} to container")]
    LinkItem {
        id: String,
        #[source]
        source: StoreError,
    },
    #[error("failed to link {} to {}", store_path.display(), dest.display())]
    Link {
        store_path: PathBuf,
        dest: PathBuf,
        #[source]
        source: LinkError,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct GetConfig {
    pub get_comments: bool,
    pub convert_html: bool,
}
impl Default for GetConfig {
    fn default() -> Self {
        Self {
            get_comments: false,
            convert_html: true,
        }
    }
}
impl Display for GetConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.get_comments {
            f.write_str("+comment")
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
pub struct ContainerItem<I> {
    pub processed: bool,
    pub value: I,
}

pub struct Driver {
    pub client: Client,
    pub store: Store,
    initialized: bool,
}
impl Driver {
    pub fn create<P: AsRef<Path>>(store_path: P) -> Result<Self, StoreError> {
        Ok(Self {
            client: Client::new(),
            store: Store::create(store_path)?,
            initialized: false,
        })
    }
    pub fn open<P: AsRef<Path>>(store_path: P) -> Result<Self, StoreError> {
        Ok(Self {
            client: Client::new(),
            store: Store::open(store_path)?,
            initialized: false,
        })
    }
    pub fn save(&mut self) -> Result<(), StoreError> {
        self.store.save()
    }
    pub async fn init(&mut self) -> Result<(), reqwest::Error> {
        self.client.init().await?;
        self.initialized = true;
        Ok(())
    }
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub async fn process_item<I: Item, P: progress::ItemProg>(
        &self,
        prog: &P,
        item: &mut I,
        config: GetConfig,
    ) -> Result<(), ItemError> {
        log::info!("getting images for {} {}", I::TYPE, item.id());
        if item.get_images(&self.client, prog).await {
            prog.sleep(self.client.request_interval).await;
        }
        if config.get_comments {
            log::info!("getting comments for {} {}", I::TYPE, item.id());
            item.get_comments(&self.client, prog)
                .await
                .map_err(ItemError::from)?;
            prog.sleep(self.client.request_interval).await;
        }
        if config.convert_html {
            log::info!("converting html for {} {}", I::TYPE, item.id());
            item.convert_html();
        }
        Ok(())
    }

    async fn process_response<I, P>(
        &mut self,
        prog: &P,
        data: serde_json::Value,
        config: GetConfig,
    ) -> Result<(I, PathBuf), ItemError>
    where
        I: Item + media::HasImage + BasicStoreItem,
        P: progress::ItemProg,
    {
        let mut ret: I = I::from_reply(
            I::Reply::deserialize(&data).map_err(ItemError::from)?,
            RawData {
                info: RawDataInfo {
                    fetch_time: Utc::now(),
                    container: raw_data::Container::None,
                },
                data,
            },
        );
        self.process_item(prog, &mut ret, config).await?;
        log::info!("add item {} {} to store", I::TYPE, ret.id());
        let dest = self.store.add_object(&ret).map_err(ItemError::from)?;
        log::debug!("store path: {}", dest.display());
        self.store.add_media(&ret).map_err(ItemError::from)?;
        Ok((ret, dest))
    }

    async fn update_item_impl<'a, I, P>(
        &mut self,
        prog: &P,
        id: <I as HasId>::Id<'a>,
        config: GetConfig,
    ) -> Result<(I, PathBuf), ItemError>
    where
        I: Fetchable + Item + media::HasImage + BasicStoreItem,
        P: progress::ItemProg,
    {
        self.process_response(
            prog,
            {
                log::info!("fetching raw data for {} {}", I::TYPE, id);
                let data = I::fetch(&self.client, id).await.map_err(ItemError::from)?;
                log::trace!("raw data {:#?}", data);
                data
            },
            config,
        )
        .await
    }

    pub async fn get_item<'a, I, P>(
        &mut self,
        prog: &P,
        id: <I as HasId>::Id<'a>,
        config: GetConfig,
    ) -> Result<Option<I>, ItemError>
    where
        I: Fetchable + Item + media::HasImage + BasicStoreItem,
        P: progress::Reporter,
    {
        Ok(if <I as StoreItem>::in_store(id, &self.store) {
            None
        } else {
            let p = prog.start_item("Getting", "", I::TYPE, id, config);
            let ret = self.update_item_impl(&p, id, config).await?.0;
            p.finish("Got", id);
            Some(ret)
        })
    }
    pub async fn add_raw_item<I, P>(
        &mut self,
        prog: &P,
        data: serde_json::Value,
        config: GetConfig,
    ) -> Result<I, ItemError>
    where
        I: Item + media::HasImage + BasicStoreItem,
        P: progress::ItemProg,
    {
        self.process_response::<I, _>(prog, data, config)
            .await
            .map(|v| v.0)
    }

    pub async fn download_item<'a, I, P, Pat>(
        &mut self,
        prog: &P,
        id: <I as HasId>::Id<'a>,
        config: GetConfig,
        relative: bool,
        dest: Pat,
    ) -> Result<Option<I>, ItemError>
    where
        I: Fetchable + Item + media::HasImage + BasicStoreItem,
        P: progress::Reporter,
        Pat: AsRef<Path>,
    {
        let canon_dest = prepare_dest(dest.as_ref()).map_err(ItemError::DestPrep)?;
        let (v, store_path) = if <I as StoreItem>::in_store(id, &self.store) {
            (None, self.store.store_path::<I>(id))
        } else {
            let p = prog.start_item("Downloading", "", I::TYPE, id, config);
            let (v, sp) = self.update_item_impl::<I, _>(&p, id, config).await?;
            p.finish("Downloaded", id);
            (Some(v), sp)
        };
        log::info!(
            "link {} {} ({}) to {}",
            I::TYPE,
            id,
            store_path.display(),
            canon_dest.display()
        );
        link_to_dest(relative, store_path.as_path(), &canon_dest).map_err(|e| ItemError::Link {
            store_path,
            dest: canon_dest,
            source: e,
        })?;
        prog.link_item(I::TYPE, id, dest);
        Ok(v)
    }
    pub async fn update_item<'a, I, P>(
        &mut self,
        prog: &P,
        id: <I as HasId>::Id<'a>,
        config: GetConfig,
    ) -> Result<I, ItemError>
    where
        I: Fetchable + Item + media::HasImage + BasicStoreItem,
        P: progress::Reporter,
    {
        let p = prog.start_item("Updating", "", I::TYPE, id, config);
        let ret = self.update_item_impl(&p, id, config).await?;
        p.finish("Updated", id);
        Ok(ret.0)
    }

    async fn update_container_impl<'a, IC, I, O, P>(
        &mut self,
        prog: &P,
        id: IC::Id<'_>,
        config: GetConfig,
    ) -> Result<(Vec<ContainerItem<I>>, PathBuf), ContainerError>
    where
        I: Item + StoreItem + media::HasImage,
        IC: ItemContainer<O, I>,
        P: progress::ItemContainerProg,
    {
        log::info!(
            "fetching container items for {} in {} {} ({})",
            I::TYPE,
            IC::TYPE,
            id,
            IC::OPTION_NAME
        );
        let dat = IC::fetch_items(&self.client, prog, id)
            .await
            .map_err(ContainerError::from)?;
        let mut ret = Vec::with_capacity(dat.len());
        {
            let mut p = prog.start_items(dat.len() as u64);
            for (idx, i) in dat.into_iter().enumerate() {
                use progress::ItemsProg;
                log::info!("parsing api response of {}", idx);
                log::trace!("api response {:#?}", i);
                let mut item = IC::parse_item(i).map_err(ContainerError::from)?;
                if I::in_store(item.id(), &self.store) {
                    ret.push(ContainerItem {
                        processed: false,
                        value: item,
                    });
                    p.skip_item();
                    continue;
                }
                let i_p = p.start_item(I::TYPE, item.id());
                self.process_item(&i_p, &mut item, config)
                    .await
                    .map_err(|e| ContainerError::Item {
                        id: item.id().to_string(),
                        source: e,
                    })?;
                log::info!("add {} {} to store", I::TYPE, item.id());
                if let Some(v) =
                    item.save_data(&mut self.store)
                        .map_err(|e| ContainerError::Item {
                            id: item.id().to_string(),
                            source: ItemError::Store(e),
                        })?
                {
                    log::debug!("store path: {}", v.display());
                }
                self.store
                    .add_media(&item)
                    .map_err(|e| ContainerError::Item {
                        id: item.id().to_string(),
                        source: ItemError::Media(e),
                    })?;
                log::info!(
                    "finished processing {} {} in {} {} ({})",
                    I::TYPE,
                    item.id(),
                    IC::TYPE,
                    id,
                    IC::OPTION_NAME
                );
                ret.push(ContainerItem {
                    processed: true,
                    value: item,
                });
            }
        }
        let container = self
            .store
            .add_container::<IC, O, I>(id)
            .map_err(ContainerError::from)?;
        for i in ret.iter() {
            container
                .link_item(i.value.id())
                .map_err(|e| ContainerError::LinkItem {
                    id: i.value.id().to_string(),
                    source: e,
                })?;
        }
        let sp = container.finish();
        Ok((ret, sp))
    }

    pub async fn get_container<'a, IC, I, O, P>(
        &mut self,
        prog: &P,
        id: <IC as HasId>::Id<'a>,
        config: GetConfig,
    ) -> Result<Option<Vec<ContainerItem<I>>>, ContainerError>
    where
        I: Item + StoreItem + media::HasImage,
        IC: ItemContainer<O, I>,
        P: progress::Reporter,
    {
        if IC::in_store(id, &self.store.containers) {
            Ok(None)
        } else {
            let p = prog.start_item_container::<I, O, IC, _, _>("Getting", "", id, config);
            let (ret, _) = self
                .update_container_impl::<IC, I, O, _>(&p, id, config)
                .await?;
            p.finish("Got", Some(ret.len()), id);
            Ok(Some(ret))
        }
    }
    pub async fn update_container<'a, IC, I, O, P>(
        &mut self,
        prog: &P,
        id: <IC as HasId>::Id<'a>,
        config: GetConfig,
    ) -> Result<Vec<ContainerItem<I>>, ContainerError>
    where
        I: Item + StoreItem + media::HasImage,
        IC: ItemContainer<O, I>,
        P: progress::Reporter,
    {
        let p = prog.start_item_container::<I, O, IC, _, _>("Updating", "", id, config);
        let (r, _) = self
            .update_container_impl::<IC, I, O, _>(&p, id, config)
            .await?;
        p.finish("Updated", Some(r.len()), id);
        Ok(r)
    }
    pub async fn download_container<'a, IC, I, O, P, Pat>(
        &mut self,
        prog: &P,
        id: <IC as HasId>::Id<'a>,
        config: GetConfig,
        relative: bool,
        dest: Pat,
    ) -> Result<Option<Vec<ContainerItem<I>>>, ContainerError>
    where
        I: Item + StoreItem + media::HasImage,
        IC: ItemContainer<O, I>,
        P: progress::Reporter,
        Pat: AsRef<Path>,
    {
        let canon_dest = prepare_dest(dest.as_ref()).map_err(ContainerError::from)?;
        let (ret, store_path) = if IC::in_store(id, &self.store.containers) {
            (None, self.store.container_store_path::<IC, O, I>(id))
        } else {
            let p = prog.start_item_container::<I, O, IC, _, _>("Downloading", "", id, config);
            let (v, sp) = self
                .update_container_impl::<IC, I, O, _>(&p, id, config)
                .await?;
            p.finish("Downloaded", Some(v.len()), id);
            (Some(v), sp)
        };
        link_to_dest(relative, store_path.as_path(), canon_dest.as_path()).map_err(|e| {
            ContainerError::Link {
                store_path,
                dest: dest.as_ref().to_path_buf(),
                source: e,
            }
        })?;
        prog.link_container::<I, O, IC, _, _>(id, dest);
        Ok(ret)
    }
}
