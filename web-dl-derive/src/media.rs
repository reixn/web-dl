extern crate proc_macro;

use darling::{FromAttributes, FromField, FromMeta, FromVariant, ToTokens};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, Index};

#[derive(Clone, Copy, Default, FromMeta)]
enum StorePath {
    #[default]
    Regular,
    Flatten,
    DynExtension,
}
#[derive(FromAttributes)]
#[darling(attributes(has_image))]
struct HasImage {
    #[darling(default)]
    path: StorePath,
}

enum FieldSpec {
    HasImage(HasImage),
    Ignore,
}
impl FromField for FieldSpec {
    fn from_field(field: &syn::Field) -> darling::Result<Self> {
        if field
            .attrs
            .iter()
            .any(|a| a.path.to_token_stream().to_string() == "has_image")
        {
            HasImage::from_attributes(&field.attrs).map(Self::HasImage)
        } else {
            Ok(Self::Ignore)
        }
    }
}

macro_rules! support {
    ($i:ident) => {
        quote!(::web_dl_base::media::macro_export::$i)
    };
}
macro_rules! exported {
    ($i:ident) => {
        quote!(::web_dl_base::media::$i)
    };
}

struct FieldInfo {
    name: String,
    expr: TokenStream,
    spec: HasImage,
}
fn gen_stmts<F: Fn(&str, &TokenStream, TokenStream) -> TokenStream>(
    fields: &[FieldInfo],
    gen_fn: F,
) -> TokenStream {
    if fields.is_empty() {
        let res = support!(Result);
        return quote!(#res::Ok(()));
    }
    let mut ret = TokenStream::new();
    let ext = support!(with_extension);
    for (idx, FieldInfo { name, expr, spec }) in fields.iter().enumerate() {
        let name = name.as_str();
        let expr = match spec.path {
            StorePath::Regular => gen_fn(name, expr, quote!(path.join(#name))),
            StorePath::DynExtension => {
                let call = gen_fn(name, expr, quote!(path));
                quote! {
                    {
                        let path = #ext(&#expr, path, #name);
                        #call
                    }
                }
            }
            StorePath::Flatten => gen_fn(name, expr, quote!(path)),
        };
        ret.extend(if idx == fields.len() - 1 {
            expr
        } else {
            quote!(#expr?;)
        });
    }
    ret
}
fn gen_drops(info: &[FieldInfo]) -> TokenStream {
    let mut ret = TokenStream::new();
    for i in info {
        let expr = &i.expr;
        ret.extend(quote! { #expr.drop_images(); });
    }
    ret
}

struct VariantInfo {
    ident: Ident,
    complete: bool,
    match_ident: TokenStream,
    match_expr: Ident,
}
enum VarientSpec {
    Ignore,
    Single(VariantInfo),
}
impl FromVariant for VarientSpec {
    fn from_variant(variant: &syn::Variant) -> darling::Result<Self> {
        let mut ret = None;
        for (idx, f) in variant.fields.iter().enumerate() {
            if f.attrs
                .iter()
                .any(|e| e.path.to_token_stream().to_string() == "has_image")
            {
                match ret {
                    Some(_) => panic!("one varient with one field that has image is supported"),
                    None => {
                        ret = Some(match &f.ident {
                            Some(i) => VariantInfo {
                                ident: variant.ident.clone(),
                                complete: variant.fields.len() == 1,
                                match_ident: i.to_token_stream(),
                                match_expr: i.clone(),
                            },
                            None => VariantInfo {
                                ident: variant.ident.clone(),
                                complete: variant.fields.len() == 1,
                                match_ident: Index::from(idx).to_token_stream(),
                                match_expr: format_ident!("__field_{}", idx),
                            },
                        })
                    }
                }
            }
        }
        Ok(ret.map_or(Self::Ignore, Self::Single))
    }
}

fn gen_impl(
    name: Ident,
    load_impl: TokenStream,
    migrate_impl: TokenStream,
    store_impl: TokenStream,
    drop_impl: TokenStream,
) -> proc_macro::TokenStream {
    let t_name = exported!(StoreImage);
    let res = support!(Result);
    let err = exported!(Error);
    let path = {
        let as_ref = support!(AsRef);
        let path = support!(Path);
        quote!(#as_ref<#path>)
    };
    quote! {
        impl #t_name for #name {
            fn load_images<P>(&mut self, path: P) -> #res<(), #err>
            where
                P: #path
            {
                #load_impl
            }
            fn store_images<P>(&self, path: P) -> #res<(), #err>
            where
                P:#path
            {
                #store_impl
            }
            fn migrate<S, P>(&self, image_store: S, path: P) -> #res<(), #err>
            where
                S: #path,
                P: #path
            {
                #migrate_impl
            }
            fn drop_images(&mut self) {
                #drop_impl
            }
        }
    }
    .into()
}

pub fn derive_store_image(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);

    let load_chained = support!(load_img_chained);
    let migrate_chained = support!(migrate_img_chained);
    let store_chained = support!(store_img_chained);

    match input.data {
        Data::Struct(s) => {
            let s: Vec<FieldInfo> = match s.fields {
                Fields::Named(n) => n.named,
                Fields::Unnamed(u) => u.unnamed,
                Fields::Unit => syn::punctuated::Punctuated::new(),
            }
            .into_iter()
            .enumerate()
            .filter_map(|(idx, f)| match FieldSpec::from_field(&f).unwrap() {
                FieldSpec::HasImage(spec) => Some(match &f.ident {
                    Some(i) => FieldInfo {
                        name: i.to_string(),
                        expr: quote!(self.#i),
                        spec,
                    },
                    None => FieldInfo {
                        name: idx.to_string(),
                        expr: quote!(self.#idx),
                        spec,
                    },
                }),
                FieldSpec::Ignore => None,
            })
            .collect();
            let create_dirs = support!(create_dir_missing);
            gen_impl(
                input.ident,
                {
                    let stmt = gen_stmts(
                        &s,
                        |name, expr, path| quote!(#load_chained(&mut #expr, #path, #name)),
                    );
                    quote! {
                        let path = path.as_ref();
                        #stmt
                    }
                },
                {
                    let stmt = gen_stmts(
                        &s,
                        |name, expr, path| quote!(#migrate_chained(&#expr, image_store, #path, #name)),
                    );
                    quote! {
                        let path = path.as_ref();
                        let image_store = image_store.as_ref();
                        #create_dirs(path)?;
                        #stmt
                    }
                },
                {
                    let stmt = gen_stmts(
                        &s,
                        |name, expr, path| quote!(#store_chained(&#expr, #path,#name)),
                    );
                    quote! {
                        let path = path.as_ref();
                        #create_dirs(path)?;
                        #stmt
                    }
                },
                gen_drops(&s),
            )
        }
        Data::Enum(e) => {
            let ok = {
                let res = support!(Result);
                quote!(#res::Ok(()))
            };
            let vs: Vec<VariantInfo> = e
                .variants
                .iter()
                .filter_map(|v| match VarientSpec::from_variant(v).unwrap() {
                    VarientSpec::Ignore => None,
                    VarientSpec::Single(s) => Some(s),
                })
                .collect();
            if vs.is_empty() {
                return gen_impl(
                    input.ident,
                    ok.clone(),
                    ok.clone(),
                    ok.clone(),
                    TokenStream::new(),
                );
            }
            let mut load_stmt = TokenStream::new();
            let mut migrate_stmt = TokenStream::new();
            let mut store_stmt = TokenStream::new();
            let mut drop_stmt = TokenStream::new();
            for VariantInfo {
                ident,
                complete,
                match_ident,
                match_expr,
            } in vs.iter()
            {
                let mat = if *complete {
                    quote!(Self::#ident { #match_ident: #match_expr } )
                } else {
                    quote!(Self::#ident { #match_ident: #match_expr, .. })
                };
                let name = ident.to_string();
                load_stmt.extend(quote!(#mat => #load_chained(#match_expr, path, #name),));
                migrate_stmt.extend(
                    quote!(#mat => #migrate_chained(#match_expr, image_store, path, #name),),
                );
                store_stmt.extend(quote!(#mat => #store_chained(#match_expr,  path, #name),));
                drop_stmt.extend(quote!(#mat => #match_expr.drop_images(),));
            }
            if vs.len() != e.variants.len() {
                load_stmt.extend(quote!(_ => #ok));
                migrate_stmt.extend(quote!(_ => #ok));
                store_stmt.extend(quote!(_ => #ok));
                drop_stmt.extend(quote!(_ => ()));
            }
            gen_impl(
                input.ident,
                quote!(match self { #load_stmt }),
                quote!(match self {#migrate_stmt}),
                quote!(match self {#store_stmt}),
                quote!(match self {#drop_stmt}),
            )
        }
        Data::Union(_) => panic!("derive StoreImage for union is not supported"),
    }
}
