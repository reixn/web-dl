use std::{fmt::Display, path::Path};
pub use web_dl_base::progress::{ImageProg, ImagesProg, Progress};

use crate::item;

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

pub trait ItemJob: ItemProg {
    fn finish<I: Display>(self, operation: &str, id: I);
}
pub trait ContainerJob: ItemContainerProg {
    fn finish<I: Display>(self, operation: &str, num: Option<usize>, id: I);
}
pub trait Reporter: Progress {
    fn new(jobs: Option<u64>) -> Self;

    type ItemRep<'a>: ItemJob
    where
        Self: 'a;
    fn start_item<O: Display, I: Display>(
        &self,
        operation: &str,
        prefix: &'static str,
        kind: &'static str,
        id: I,
        option: O,
    ) -> Self::ItemRep<'_>;
    fn link_item<I: Display, P: AsRef<Path>>(&self, kind: &str, id: I, dest: P);

    type ItemContainerRep<'a>: ContainerJob
    where
        Self: 'a;
    fn start_item_container<II, IO, IC, I, O>(
        &self,
        operation: &str,
        prefix: &'static str,
        id: I,
        option: O,
    ) -> Self::ItemContainerRep<'_>
    where
        II: item::Item,
        IC: item::ItemContainer<IO, II>,
        I: Display,
        O: Display;
    fn link_container<II, IO, IC, I, P>(&self, id: I, dest: P)
    where
        II: item::Item,
        IC: item::ItemContainer<IO, II>,
        I: Display,
        P: AsRef<Path>;
}

pub mod progress_bar;
pub mod silent;
