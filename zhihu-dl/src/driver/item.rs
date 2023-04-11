use super::{Driver, GetConfig};
use crate::{
    element::comment,
    item::{Fetchable, Item},
    progress::{self, ItemJob},
    raw_data::{self, RawData, RawDataInfo},
    store::{BasicStoreItem, StoreItem},
    util::relative_path::{link_to_dest, prepare_dest, DestPrepError, LinkError},
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
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

impl Driver {
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
        if config.get_comments && item.has_comment() {
            log::info!("getting comments for {} {}", I::TYPE, item.id());
            item.get_comments(prog.start_comment_tree(), &self.client)
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
        I: Item + BasicStoreItem,
        P: progress::ItemProg,
    {
        let mut ret: I = I::from_reply(
            I::Reply::deserialize(&data).map_err(ItemError::from)?,
            RawData {
                info: RawDataInfo {
                    fetch_time: chrono::Utc::now(),
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
        I: Fetchable + Item + BasicStoreItem,
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
        I: Fetchable + Item + BasicStoreItem,
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
        I: Item + BasicStoreItem,
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
        I: Fetchable + Item + BasicStoreItem,
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
        I: Fetchable + Item + BasicStoreItem,
        P: progress::Reporter,
    {
        let p = prog.start_item("Updating", "", I::TYPE, id, config);
        let ret = self.update_item_impl(&p, id, config).await?;
        p.finish("Updated", id);
        Ok(ret.0)
    }
}
