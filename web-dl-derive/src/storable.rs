extern crate proc_macro;

use crate::attrib::{FieldSpec, StoreFormat, StorePath};
use darling::{ast::Data, FromDeriveInput};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    token::{Colon, Comma},
    DeriveInput, Expr, FieldValue, Ident, Member,
};

#[derive(FromDeriveInput)]
#[darling(attributes(store))]
struct InputRecv {
    ident: Ident,
    #[darling(default)]
    format: StoreFormat,
    data: Data<(), FieldSpec>,
}

macro_rules! external {
    ($i:ident) => {
        quote!(::web_dl_base::storable::macro_export::$i)
    };
}
macro_rules! exported {
    ($i:ident) => {
        quote!(::web_dl_base::storable::$i)
    };
}

fn gen_impl(name: Ident, load: TokenStream, store: TokenStream) -> proc_macro::TokenStream {
    let p = external!(Path);
    let as_ref = external!(AsRef);
    let res = external!(Result);
    let t_name = exported!(Storable);
    let opt = exported!(LoadOpt);
    let err = exported!(Error);
    quote! {
        impl #t_name for #name {
            fn load<P:#as_ref<#p>>(path: P, __load_opt: #opt) -> #res<Self, #err> {
                #load
            }
            fn store<P:#as_ref<#p>>(&self, path: P) -> #res<(), #err> {
                #store
            }
        }
    }
    .into()
}

pub fn derive_storable(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = InputRecv::from_derive_input(&parse_macro_input!(input as DeriveInput)).unwrap();
    match input.format {
        StoreFormat::Directory => {
            let res = external!(Result);
            let load_chain = exported!(load_chained);
            let store_chain = exported!(store_chained);
            let mut load_fields: Punctuated<FieldValue, Comma> = Punctuated::new();
            let mut store_fields = Vec::new();
            for i in input.data.take_struct().unwrap() {
                let id = i.ident.unwrap();
                let id_str = id.to_string();
                let path: Expr = match i.path {
                    StorePath::Regular => {
                        parse_quote!(path.with_file_name(#id_str))
                    }
                    StorePath::Flatten => parse_quote!(path),
                    StorePath::Ext(e) => {
                        let v = format!("{}.{}", id_str, e);
                        parse_quote!(path.with_file_name(#v))
                    }
                    StorePath::Name(r) => parse_quote!(path.with_file_name(#r)),
                };
                load_fields.push({
                    let load_expr: Expr = parse_quote!(#load_chain(#path, __load_opt, #id_str)?);
                    FieldValue {
                        attrs: Vec::new(),
                        member: Member::Named(id.clone()),
                        colon_token: Some(Colon::default()),
                        expr: if i.raw_data.is_present() {
                            parse_quote! {
                                if __load_opt.load_raw {
                                    Some(#load_expr)
                                } else {
                                    None
                                }
                            }
                        } else {
                            load_expr
                        },
                    }
                });
                store_fields.push(quote! {
                    #store_chain(&self.#id, #path, #id_str)
                });
            }
            let create_dir = exported!(create_dir_missing);
            gen_impl(
                input.ident,
                quote! {
                    let path = path.as_ref();
                    #res::Ok(Self { #load_fields })
                },
                {
                    if store_fields.is_empty() {
                        quote! {#res::Ok(())}
                    } else {
                        let mut store_stmt = TokenStream::new();
                        for i in &store_fields[0..store_fields.len() - 1] {
                            store_stmt.extend(quote!(#i?;));
                        }
                        let last = store_fields.last().unwrap();
                        quote! {
                            let path = path.as_ref();
                            #create_dir(path)?;
                            #store_stmt
                            return #last;
                        }
                    }
                },
            )
        }
        StoreFormat::Yaml => {
            let load = exported!(load_yaml);
            let store = exported!(store_yaml);
            gen_impl(
                input.ident,
                quote! {#load(path)},
                quote! {#store(self, path)},
            )
        }
        StoreFormat::Json => {
            let load = exported!(load_json);
            let store = exported!(store_json);
            gen_impl(
                input.ident,
                quote! {#load(path)},
                quote! {#store(self, path)},
            )
        }
    }
}
