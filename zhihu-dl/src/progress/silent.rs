use super::*;
use std::fmt::Display;

pub struct Silent;
impl Progress for Silent {
    async fn sleep(&self, duration: std::time::Duration) {
        tokio::time::sleep(duration).await
    }
}
impl FetchProg for Silent {
    fn set_count(&mut self, _: Option<u64>) {}
    fn inc(&mut self, _: u64) {}
}

impl ImageProg for Silent {
    fn set_size(&mut self, _: Option<u64>) {}
    fn inc(&mut self, _: u64) {}
}
impl ImagesProg for Silent {
    type ImageRep<'a> = Silent;
    fn start_image<I: Display>(&mut self, _: I) -> Self::ImageRep<'_> {
        Silent
    }
    fn skip(&mut self) {}
}

impl CommentProg for Silent {
    type ChildRep<'a> = Silent;
    fn start_child(&self) -> Self::ChildRep<'_> {
        Silent
    }

    type ImagesRep<'a> = Silent;
    fn start_images(&self, _: u64) -> Self::ImagesRep<'_> {
        Silent
    }
}
impl CommentsProg for Silent {
    type CommentRep<'a> = Silent;
    fn start_comment<I: Display>(&mut self, _: I) -> Self::CommentRep<'_> {
        Silent
    }
    fn skip_comment(&mut self) {}
}
impl CommentTreeProg for Silent {
    type FetchRep<'a> = Silent;
    fn start_fetch_root(&self) -> Self::FetchRep<'_> {
        Silent
    }

    type FetchMissingRep<'a> = Silent;
    fn start_fetch_missing(&self) -> Self::FetchMissingRep<'_> {
        Silent
    }

    type CommentsRep<'a> = Silent;
    fn start_comments(&self, _: u64) -> Self::CommentsRep<'_> {
        Silent
    }
}

impl ItemProg for Silent {
    type CommentTreeRep<'a> = Silent;
    fn start_comment_tree(&self) -> Self::CommentTreeRep<'_> {
        Silent
    }

    type ImagesRep<'a> = Silent;
    fn start_images(&self, _: u64) -> Self::ImagesRep<'_> {
        Silent
    }
}
impl ItemsProg for Silent {
    type ItemRep<'a> = Silent;
    fn start_item<I: Display>(&mut self, _: &str, _: I) -> Self::ItemRep<'_> {
        Silent
    }
    fn skip_item(&mut self) {}
}
impl ItemContainerProg for Silent {
    type FetchRep<'a> = Silent;
    fn start_fetch(&self) -> Self::FetchRep<'_> {
        Silent
    }

    type ItemsRep<'a> = Silent;
    fn start_items(&self, _: u64) -> Self::ItemsRep<'_> {
        Silent
    }
}

impl ItemJob for Silent {
    fn finish<I: Display>(self, _: &str, _: I) {}
}
impl ContainerJob for Silent {
    fn finish<I: Display>(self, _: &str, _: Option<usize>, _: I) {}
}

impl Reporter for Silent {
    type ItemRep<'a> = Silent;
    fn start_item<O, I>(&self, _: &str, _: &str, _: &str, _: I, _: Option<O>) -> Self::ItemRep<'_> {
        Silent
    }
    fn link_item<I, P>(&self, _: &str, _: I, _: P) {}

    type ItemContainerRep<'a> = Silent;
    fn start_item_container<II, IO, IC, I, O>(
        &self,
        _: &str,
        _: &'static str,
        _: I,
        _: Option<O>,
    ) -> Self::ItemContainerRep<'_> {
        Silent
    }
    fn link_container<II, IO, IC, I, P>(&self, _: I, _: P) {}
}
