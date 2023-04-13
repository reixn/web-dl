use crate::progress::Reporter;
use std::path::Path;

pub mod spec;
pub use spec::Manifest;

pub mod leaf;

pub mod branch;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to get leaf")]
    Leaf(
        #[source]
        #[from]
        leaf::Error,
    ),
    #[error("failed to create symlinks")]
    Link(
        #[source]
        #[from]
        branch::Error,
    ),
}

impl super::Driver {
    pub async fn apply_manifest<P: Reporter, Pat: AsRef<Path>>(
        &mut self,
        prog: &P,
        manifest: &spec::Manifest,
        dest: Pat,
    ) -> Result<(), Error> {
        self.apply_manifest_leaf(prog, &manifest.merged_leaf())
            .await
            .map_err(Error::from)?;
        self.link_manifest(prog, manifest, dest)
            .map_err(Error::from)
    }
}
