use proc_macro::TokenStream;

mod media;
mod storable;

#[proc_macro_derive(StoreImage, attributes(has_image))]
pub fn derive_store_image(input: TokenStream) -> TokenStream {
    media::derive_store_image(input)
}

#[proc_macro_derive(Storable, attributes(store))]
pub fn derive_storable(input: TokenStream) -> TokenStream {
    storable::derive_storable(input)
}
