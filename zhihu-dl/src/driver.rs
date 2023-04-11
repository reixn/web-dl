use crate::{
    request::Client,
    store::{Store, StoreError},
};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, path::Path};

pub mod manifest;

mod get_config {
    #[inline]
    pub fn comments_default() -> bool {
        false
    }
    #[inline]
    pub fn convert_html_default() -> bool {
        true
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GetConfig {
    #[serde(default = "get_config::comments_default")]
    pub get_comments: bool,
    #[serde(default = "get_config::convert_html_default")]
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

pub struct Driver {
    pub client: Client,
    pub store: Store,
    initialized: bool,
}

pub mod item;
pub use item::ItemError;

pub mod container;
pub use container::ContainerError;

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
}
