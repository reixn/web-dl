use super::{Driver, ItemError};
use crate::{
    item::{Item, ItemContainer},
    progress::{self, ContainerJob},
    store::{self, ContainerHandle},
    util::relative_path::{link_to_dest, prepare_dest, DestPrepError, LinkError},
};
use std::path::{Path, PathBuf};
use web_dl_base::id::HasId;

#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    #[error("failed to store container")]
    Store(
        #[source]
        #[from]
        store::StoreError,
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
        source: store::StoreError,
    },
    #[error("failed to link {} to {}", store_path.display(), dest.display())]
    Link {
        store_path: PathBuf,
        dest: PathBuf,
        #[source]
        source: LinkError,
    },
}

#[derive(Debug)]
pub struct ContainerItem<I> {
    pub processed: bool,
    pub value: I,
}

impl Driver {
    pub(super) async fn fetch_container<'a, IC, I, O, P>(
        &self,
        prog: &P,
        id: IC::Id<'_>,
    ) -> Result<impl Iterator<Item = Result<I, ContainerError>> + ExactSizeIterator, ContainerError>
    where
        IC: ItemContainer<O, I>,
        I: Item,
        P: progress::ItemContainerProg,
    {
        Ok(IC::fetch_items(&self.client, prog, id)
            .await
            .map_err(ContainerError::from)?
            .into_iter()
            .enumerate()
            .map(|(idx, i)| {
                log::info!("parsing api response of {}", idx);
                log::trace!("api response {:#?}", i);
                IC::parse_item(i).map_err(ContainerError::from)
            }))
    }
    async fn update_container_impl<'a, IC, I, O, P>(
        &mut self,
        prog: &P,
        id: IC::Id<'_>,
    ) -> Result<(Vec<ContainerItem<I>>, Option<PathBuf>), ContainerError>
    where
        I: Item,
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
        let dat = self.fetch_container::<IC, I, O, _>(prog, id).await?;
        let mut ret = Vec::with_capacity(dat.len());
        {
            let mut p = prog.start_items(dat.len() as u64);
            for item in dat {
                use progress::ItemsProg;
                let mut item = item?;
                if I::in_store(item.id(), &self.store).in_store {
                    ret.push(ContainerItem {
                        processed: false,
                        value: item,
                    });
                    p.skip_item();
                    continue;
                }
                let i_p = p.start_item(I::TYPE, item.id());
                self.process_item(&i_p, &mut item).await;
                log::info!("add {} {} to store", I::TYPE, item.id());
                if let Some(v) =
                    item.save_data(true, &mut self.store)
                        .map_err(|e| ContainerError::Item {
                            id: item.id().to_string(),
                            source: ItemError::Store(e),
                        })?
                {
                    log::debug!("store path: {}", v.display());
                }
                item.add_media(&mut self.store)
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
        let mut container = IC::save_data(id, &mut self.store).map_err(ContainerError::from)?;
        for i in ret.iter() {
            container
                .link_item(i.value.id())
                .map_err(|e| ContainerError::LinkItem {
                    id: i.value.id().to_string(),
                    source: e,
                })?;
        }
        container.mark_missing();
        let sp = container.finish().map_err(ContainerError::Store)?;
        Ok((ret, sp))
    }

    pub async fn get_container<'a, IC, I, O, P>(
        &mut self,
        prog: &P,
        id: <IC as HasId>::Id<'a>,
    ) -> Result<Option<Vec<ContainerItem<I>>>, ContainerError>
    where
        I: Item,
        IC: ItemContainer<O, I>,
        P: progress::Reporter,
    {
        if IC::in_store(id, &self.store) {
            Ok(None)
        } else {
            let p = prog.start_item_container::<I, O, IC, _, &str>("Getting", "", id, None);
            let (ret, _) = self.update_container_impl::<IC, I, O, _>(&p, id).await?;
            p.finish("Got", Some(ret.len()), id);
            Ok(Some(ret))
        }
    }
    pub async fn update_container<'a, IC, I, O, P>(
        &mut self,
        prog: &P,
        id: <IC as HasId>::Id<'a>,
    ) -> Result<Vec<ContainerItem<I>>, ContainerError>
    where
        I: Item,
        IC: ItemContainer<O, I>,
        P: progress::Reporter,
    {
        let p = prog.start_item_container::<I, O, IC, _, &str>("Updating", "", id, None);
        let (r, _) = self.update_container_impl::<IC, I, O, _>(&p, id).await?;
        p.finish("Updated", Some(r.len()), id);
        Ok(r)
    }
    pub async fn download_container<'a, IC, I, O, P, Pat>(
        &mut self,
        prog: &P,
        id: <IC as HasId>::Id<'a>,
        relative: bool,
        dest: Pat,
    ) -> Result<Option<Vec<ContainerItem<I>>>, ContainerError>
    where
        I: Item,
        IC: ItemContainer<O, I>,
        P: progress::Reporter,
        Pat: AsRef<Path>,
    {
        let canon_dest = prepare_dest(dest.as_ref()).map_err(ContainerError::from)?;
        let (ret, store_path) = if IC::in_store(id, &self.store) {
            (None, IC::store_path(id, &self.store))
        } else {
            let p = prog.start_item_container::<I, O, IC, _, &str>("Downloading", "", id, None);
            let (v, sp) = self.update_container_impl::<IC, I, O, _>(&p, id).await?;
            p.finish("Downloaded", Some(v.len()), id);
            (Some(v), sp)
        };
        if let Some(store_path) = store_path {
            link_to_dest(relative, store_path.as_path(), canon_dest.as_path()).map_err(|e| {
                ContainerError::Link {
                    store_path,
                    dest: dest.as_ref().to_path_buf(),
                    source: e,
                }
            })?;
            prog.link_container::<I, O, IC, _, _>(id, dest);
        }
        Ok(ret)
    }
}
