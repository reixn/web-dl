extern crate proc_macro;

use darling::{FromAttributes, FromField, FromMeta, FromVariant, ToTokens};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident};

#[derive(Clone, Copy, Default, FromMeta)]
enum ErrSpec {
    PassThrough,
    #[default]
    Chained,
}
#[derive(FromAttributes)]
#[darling(attributes(has_image))]
struct HasImage {
    #[darling(default)]
    error: ErrSpec,
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
    is_ref: bool,
}
fn gen_expr(info: &FieldInfo, is_load: bool, is_last: bool) -> TokenStream {
    let op = {
        let name = &info.name;
        let expr = &info.expr;
        match info.spec.error {
            ErrSpec::Chained => {
                if is_load {
                    let f = support!(load_img_chained);
                    let e = if info.is_ref {
                        info.expr.clone()
                    } else {
                        quote!(&mut #expr)
                    };
                    quote!(#f(#e, __loader, #name))
                } else {
                    let f = support!(store_img_chained);
                    let e = if info.is_ref {
                        info.expr.clone()
                    } else {
                        quote!(&#expr)
                    };
                    quote!(#f(#e, __storer, #name))
                }
            }
            ErrSpec::PassThrough => {
                if is_load {
                    quote!(#expr.load_images(__loader))
                } else {
                    quote!(#expr.store_images(__storer))
                }
            }
        }
    };
    if is_last {
        quote! {return #op;}
    } else {
        quote! {#op?;}
    }
}

fn gen_exprs(fields: &Vec<FieldInfo>, is_load: bool) -> TokenStream {
    if fields.is_empty() {
        let res = support!(Result);
        return quote!(#res::Ok(()));
    }
    let mut ret = TokenStream::new();
    for f in &fields[0..fields.len() - 1] {
        ret.extend(gen_expr(f, is_load, false));
    }
    let l = fields.last().unwrap();
    ret.extend(gen_expr(l, is_load, true));
    ret
}

fn gen_refs(info: &Vec<FieldInfo>) -> TokenStream {
    let mut ret = TokenStream::new();
    for i in info {
        let expr = i.expr.clone();
        ret.extend(quote! { #expr.image_refs(__ref_set); });
    }
    ret
}
fn gen_impl(
    name: Ident,
    load_impl: TokenStream,
    store_impl: TokenStream,
    r_set_impl: TokenStream,
) -> proc_macro::TokenStream {
    let t_name = exported!(HasImage);
    let res = support!(Result);
    let err = exported!(Error);
    let loader = exported!(Loader);
    let storer = exported!(Storer);
    let r_set = exported!(RefSet);
    quote! {
        impl #t_name for #name {
            fn load_images(&mut self, __loader: &mut #loader) -> #res<(), #err> {
                #load_impl
            }
            fn store_images(&self, __storer: &mut #storer) -> #res<(), #err> {
                #store_impl
            }
            fn image_refs<'a, 'b>(&'b self, __ref_set: &'a mut #r_set<'b>)
            where
                'b:'a
            {
                #r_set_impl
            }
        }
    }
    .into()
}
fn unit_impl(name: Ident) -> proc_macro::TokenStream {
    let res = support!(Result);
    gen_impl(
        name,
        quote!(#res::Ok(())),
        quote!(#res::Ok(())),
        TokenStream::new(),
    )
}

struct VariantRecv {
    ident: Ident,
    match_ident: Vec<(TokenStream, bool)>,
    fields: Vec<FieldInfo>,
}
impl FromVariant for VariantRecv {
    fn from_variant(variant: &syn::Variant) -> darling::Result<Self> {
        let mut m = Vec::with_capacity(variant.fields.len());
        let mut f_info = Vec::with_capacity(variant.fields.len());
        for (idx, f) in variant.fields.iter().enumerate() {
            let (id, name) = match &f.ident {
                Some(i) => (i.to_token_stream(), format!("{}::{}", variant.ident, i)),
                None => (
                    format_ident!("__field_{}", idx).to_token_stream(),
                    format!("{}::{}", variant.ident, idx),
                ),
            };
            m.push((id.clone(), f.ident.is_some()));
            FieldSpec::from_field(f).map(|fr| match fr {
                FieldSpec::HasImage(spec) => {
                    f_info.push(FieldInfo {
                        name,
                        expr: id,
                        spec,
                        is_ref: true,
                    });
                }
                FieldSpec::Ignore => (),
            })?;
        }
        Ok(Self {
            ident: variant.ident.clone(),
            match_ident: m,
            fields: f_info,
        })
    }
}

pub fn derive_has_image(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);

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
                        is_ref: false,
                    },
                    None => FieldInfo {
                        name: idx.to_string(),
                        expr: quote!(self.#idx),
                        spec,
                        is_ref: false,
                    },
                }),
                FieldSpec::Ignore => None,
            })
            .collect();
            gen_impl(
                input.ident,
                gen_exprs(&s, true),
                gen_exprs(&s, false),
                gen_refs(&s),
            )
        }
        Data::Enum(e) => {
            if e.variants.is_empty() {
                return unit_impl(input.ident);
            }
            let res = support!(Result);
            let mut load_image = TokenStream::new();
            let mut store_image = TokenStream::new();
            let mut image_ref = TokenStream::new();
            for v in e.variants.into_iter() {
                let VariantRecv {
                    ident: vid,
                    match_ident,
                    fields,
                } = VariantRecv::from_variant(&v).unwrap();
                if fields.is_empty() {
                    load_image.extend(quote!(Self::#vid { .. } => #res::Ok(())));
                    store_image.extend(quote!(Self::#vid { .. } => #res::Ok(())));
                    image_ref.extend(quote!(Self::#vid { .. } => ()));
                    continue;
                }
                let matched = {
                    let mut ret = TokenStream::new();
                    let mut named = false;
                    let mut first = true;
                    for (id, n) in match_ident {
                        if !first {
                            ret.extend(quote!(,));
                        }
                        first = false;
                        named |= n;
                        ret.extend(id);
                    }
                    if named {
                        quote!(Self::#vid{ #ret } =>)
                    } else {
                        quote!(Self::#vid(#ret) =>)
                    }
                };
                let load_expr = gen_exprs(&fields, true);
                let store_expr = gen_exprs(&fields, false);
                let r = gen_refs(&fields);
                load_image.extend(quote!(#matched { #load_expr }));
                store_image.extend(quote!(#matched { #store_expr }));
                image_ref.extend(quote!( #matched { #r } ));
            }
            gen_impl(
                input.ident,
                quote!( match self { #load_image } ),
                quote!(match self { #store_image }),
                quote!( match self { #image_ref } ),
            )
        }
        Data::Union(_) => panic!("derive HasImage for union is not supported"),
    }
}
