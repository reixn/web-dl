extern crate proc_macro;

use proc_macro::TokenStream;

mod content;

#[proc_macro_derive(HasContent, attributes(content))]
pub fn derive_has_content(input: TokenStream) -> TokenStream {
    content::derive_has_content(input)
}
