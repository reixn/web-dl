use std::fmt::Display;

pub trait Progress {
    fn suspend<F: FnOnce() -> ()>(&self, f: F);
    async fn sleep(&self, duration: std::time::Duration);
}

pub trait ImageProg: Progress {
    fn set_size(&mut self, size: Option<u64>);
    fn inc(&mut self, delta: u64);
}
pub trait ImagesProg: Progress {
    type ImageRep<'a>: ImageProg
    where
        Self: 'a;
    fn start_image<I: Display>(&mut self, url: I) -> Self::ImageRep<'_>;
    fn skip(&mut self);
}
