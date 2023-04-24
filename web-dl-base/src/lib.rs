#![feature(adt_const_params)]
#![feature(async_fn_in_trait)]
#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(maybe_uninit_slice)]
#![feature(maybe_uninit_write_slice)]
#![allow(incomplete_features)]

pub mod id;
pub mod media;
pub mod progress;
pub mod storable;
pub mod util {
    pub mod serde {
        pub mod byte_array;
        pub mod bytes;
    }
}
