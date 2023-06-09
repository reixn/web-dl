use std::{fmt::Display, path::Path};
pub use web_dl_base::progress::{ImageProg, ImagesProg, Progress};

use crate::item;

pub trait FetchProg: Progress {
    fn set_count(&mut self, count: Option<u64>);
    fn inc(&mut self, delta: u64);
}

pub trait ItemProg: Progress {
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
pub trait OtherJob {
    fn finish<I: Display>(self, operation: &str, msg: I);
}
pub trait Reporter: Progress {
    type ItemRep<'a>: ItemJob
    where
        Self: 'a;
    fn start_item<O: Display, I: Display>(
        &self,
        operation: &str,
        prefix: &'static str,
        kind: &'static str,
        id: I,
        option: Option<O>,
    ) -> Self::ItemRep<'_>;
    fn link_item<I: Display, P: AsRef<Path>>(&self, kind: &str, id: I, dest: P);

    type ItemContainerRep<'a>: ContainerJob + Reporter
    where
        Self: 'a;
    fn start_item_container<II, IO, IC, I, O>(
        &self,
        operation: &str,
        prefix: &'static str,
        id: I,
        option: Option<O>,
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

    type JobRep<'a>: OtherJob
    where
        Self: 'a;
    fn start_job<I: Display>(&self, operation: &str, msg: I) -> Self::JobRep<'_>;
}

pub mod progress_bar;
pub mod silent;
