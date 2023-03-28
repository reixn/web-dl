#![feature(iterator_try_collect)]
#![feature(async_fn_in_trait)]
#![feature(async_closure)]
#![feature(adt_const_params)]
#![feature(const_cmp)]
#![feature(const_trait_impl)]
#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(maybe_uninit_slice)]
#![feature(maybe_uninit_write_slice)]

pub mod element {
    pub mod author;
    pub mod comment;
    pub mod content;
    pub mod image;

    pub use self::{
        author::Author,
        comment::Comment,
        content::Content,
        image::{Image, ImageRef},
    };
}
pub(crate) mod bytes;
pub mod id {
    use std::fmt::Display;

    #[derive(Debug, Clone, Copy)]
    pub struct Fixed<const T: &'static str>;
    impl<const T: &'static str> Display for Fixed<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(T)
        }
    }
    pub trait HasId {
        const TYPE: &'static str;
        type Id<'a>: Display + Clone + Copy
        where
            Self: 'a;
        fn id(&self) -> Self::Id<'_>;
    }
}
pub mod item;
pub mod progress;
pub mod raw_data;
pub mod request;
pub mod store {
    pub mod storable;
}
pub mod meta;
