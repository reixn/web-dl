use darling::{
    util::{Flag, Override},
    FromField, FromMeta,
};
use syn::Ident;

#[derive(Clone, Copy, FromMeta)]
pub enum ImgErrSpec {
    PassThrough,
    Chained,
}
impl Default for ImgErrSpec {
    fn default() -> Self {
        ImgErrSpec::Chained
    }
}

#[derive(Clone, Copy, FromMeta)]
#[darling(default)]
pub struct HasImage {
    #[darling(default)]
    pub error: ImgErrSpec,
}
impl Default for HasImage {
    fn default() -> Self {
        Self {
            error: ImgErrSpec::default(),
        }
    }
}

#[derive(FromMeta, Clone, Copy)]
pub enum StoreFormat {
    Directory,
    Yaml,
    Json,
}
impl Default for StoreFormat {
    fn default() -> Self {
        StoreFormat::Directory
    }
}

#[derive(FromMeta)]
pub enum StorePath {
    Regular,
    Flatten,
    Ext(String),
    Name(String),
}
impl Default for StorePath {
    fn default() -> Self {
        StorePath::Regular
    }
}

#[derive(FromField)]
#[darling(attributes(store))]
pub struct FieldSpec {
    pub ident: Option<Ident>,
    #[darling(default)]
    pub path: StorePath,
    pub raw_data: Flag,
    pub has_image: Option<Override<HasImage>>,
}
