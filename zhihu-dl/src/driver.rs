use crate::{
    request::Client,
    store::{Store, StoreError},
};
use std::path::Path;

pub mod manifest;

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
