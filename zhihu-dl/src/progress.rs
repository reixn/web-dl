use std::fmt::Display;
pub use web_dl_base::progress::{ImageProg, ImagesProg, Progress};

pub trait FetchProg: Progress {
    fn set_count(&mut self, count: Option<u64>);
    fn inc(&mut self, delta: u64);
}

pub trait CommentProg: Progress {
    type ChildRep<'a>: FetchProg
    where
        Self: 'a;
    fn start_child(&self) -> Self::ChildRep<'_>;

    type ImagesRep<'a>: ImagesProg
    where
        Self: 'a;
    fn start_images(&self, count: u64) -> Self::ImagesRep<'_>;
}
pub trait CommentsProg: Progress {
    type CommentRep<'a>: CommentProg
    where
        Self: 'a;

    fn start_comment<I: Display>(&mut self, id: I) -> Self::CommentRep<'_>;
    fn skip_comment(&mut self);
}
pub trait CommentTreeProg: Progress {
    type FetchRep<'a>: FetchProg
    where
        Self: 'a;
    fn start_fetch_root(&self) -> Self::FetchRep<'_>;

    type FetchMissingRep<'a>: Progress
    where
        Self: 'a;
    fn start_fetch_missing(&self) -> Self::FetchMissingRep<'_>;

    type CommentsRep<'a>: CommentsProg
    where
        Self: 'a;
    fn start_comments(&self, count: u64) -> Self::CommentsRep<'_>;
}

pub trait ItemProg: Progress {
    type CommentTreeRep<'a>: CommentTreeProg
    where
        Self: 'a;
    fn start_comment_tree(&self) -> Self::CommentTreeRep<'_>;

    type ImagesRep<'a>: ImagesProg
    where
        Self: 'a;
    fn start_images(&self, count: u64) -> Self::ImagesRep<'_>;
}
pub trait ItemsProg: Progress {
    type ItemRep<'a>: ItemProg
    where
        Self: 'a;
    fn start_item<I: Display>(&mut self, kind: &str, id: I) -> Self::ItemRep<'_>;
    fn skip_item(&mut self);
}
pub trait ItemContainerProg: Progress {
    type FetchRep<'a>: FetchProg
    where
        Self: 'a;
    fn start_fetch(&self) -> Self::FetchRep<'_>;

    type ItemsRep<'a>: ItemsProg
    where
        Self: 'a;
    fn start_items(&self, count: u64) -> Self::ItemsRep<'_>;
}
pub trait Reporter: Progress {
    fn new(jobs: Option<u64>) -> Self;

    type ItemRep<'a>: ItemProg
    where
        Self: 'a;
    fn start_item<I: Display>(&self, kind: &str, id: I) -> Self::ItemRep<'_>;

    type ItemContainerRep<'a>: ItemContainerProg
    where
        Self: 'a;
    fn start_item_container<I: Display>(
        &self,
        kind: &str,
        id: I,
        item_kind: &str,
    ) -> Self::ItemContainerRep<'_>;
}

pub mod progress_bar;
pub mod silent;
