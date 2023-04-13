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
#![feature(slice_group_by)]
#![feature(return_position_impl_trait_in_trait)]
#![allow(incomplete_features)]

pub mod element {
    pub mod author;
    pub mod content;

    pub use self::{author::Author, content::Content};
}

#[macro_use]
pub mod store;

pub mod driver;
pub mod item;
pub mod meta;
pub mod progress;
pub mod raw_data;
pub mod request;

pub mod util {
    pub mod relative_path;
}
