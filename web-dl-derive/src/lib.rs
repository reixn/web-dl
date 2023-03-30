#![feature(iterator_try_collect)]
use proc_macro::TokenStream;

mod attrib;
mod media;
mod storable;

#[proc_macro_derive(HasImage, attributes(store))]
pub fn derive_has_image(input: TokenStream) -> TokenStream {
    media::derive_has_image(input)
}

#[proc_macro_derive(Storable, attributes(store))]
pub fn derive_storable(input: TokenStream) -> TokenStream {
    storable::derive_storable(input)
}
