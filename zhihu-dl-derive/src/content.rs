extern crate proc_macro;

use darling::{util::Flag, FromAttributes, FromField, FromVariant, ToTokens};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident};

#[derive(FromAttributes)]
#[darling(attributes(content))]
struct Content {
    main: Flag,
}

enum FieldSpec {
    Content(Content),
    Ignore,
}
impl FromField for FieldSpec {
    fn from_field(field: &syn::Field) -> darling::Result<Self> {
        if field
            .attrs
            .iter()
            .any(|a| a.path.to_token_stream().to_string() == "content")
        {
            Content::from_attributes(&field.attrs).map(Self::Content)
        } else {
            Ok(Self::Ignore)
        }
    }
}

struct FieldInfo<M> {
    match_field: M,
    match_ident: M,
    expr: TokenStream,
    spec: Content,
}
fn gen_has_converted<M>(spec: &[FieldInfo<M>]) -> TokenStream {
    if spec.is_empty() {
        return quote!(true);
    }
    let mut ret = TokenStream::new();
    let mut first = true;
    for i in spec {
        let expr = &i.expr;
        if first {
            ret.extend(quote!(#expr.is_html_converted()));
            first = false;
        } else {
            ret.extend(quote!(&& #expr.is_html_converted()));
        }
    }
    ret
}
fn gen_convert<M>(spec: &[FieldInfo<M>]) -> TokenStream {
    let mut ret = TokenStream::new();
    for i in spec {
        let expr = &i.expr;
        ret.extend(quote! {#expr.convert_html();});
    }
    ret
}
fn gen_main_content<M>(spec: &[FieldInfo<M>]) -> (TokenStream, Option<&'_ FieldInfo<M>>) {
    let mut expr = None;
    for i in spec {
        if i.spec.main.is_present() {
            if expr.is_some() {
                panic!("duplicate main content")
            }
            expr = Some((&i.expr, i));
        }
    }
    match expr {
        Some((e, i)) => (quote!(#e.get_main_content()), Some(i)),
        None => (quote!(None), None),
    }
}

fn gen_impl(
    name: Ident,
    has_convert: TokenStream,
    convert: TokenStream,
    main_content: TokenStream,
) -> proc_macro::TokenStream {
    quote! {
        impl crate::element::content::HasContent for #name {
            fn is_html_converted(&self) -> bool {
                #has_convert
            }
            fn convert_html(&mut self) {
                #convert
            }
            fn get_main_content(&self) -> Option<&'_ crate::element::content::Content> {
                #main_content
            }
        }
    }
    .into()
}

struct VariantRecv {
    ident: Ident,
    complete: bool,
    fields: Vec<FieldInfo<TokenStream>>,
}
impl FromVariant for VariantRecv {
    fn from_variant(variant: &syn::Variant) -> darling::Result<Self> {
        let mut fields = Vec::with_capacity(variant.fields.len());
        for (idx, f) in variant.fields.iter().enumerate() {
            if let FieldSpec::Content(spec) = FieldSpec::from_field(f)? {
                match &f.ident {
                    Some(i) => {
                        let tok = i.to_token_stream();
                        fields.push(FieldInfo {
                            match_field: tok.clone(),
                            match_ident: tok.clone(),
                            expr: tok,
                            spec,
                        });
                    }
                    None => {
                        let tok = format_ident!("__field_{}", idx).into_token_stream();
                        fields.push(FieldInfo {
                            match_field: syn::Index::from(idx).into_token_stream(),
                            match_ident: tok.clone(),
                            expr: tok,
                            spec,
                        });
                    }
                }
            }
        }
        Ok(Self {
            ident: variant.ident.clone(),
            complete: fields.len() == variant.fields.len(),
            fields,
        })
    }
}

pub fn derive_has_content(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    match input.data {
        Data::Struct(s) => {
            let s: Vec<FieldInfo<()>> = match s.fields {
                Fields::Named(n) => n.named,
                Fields::Unnamed(u) => u.unnamed,
                Fields::Unit => syn::punctuated::Punctuated::new(),
            }
            .into_iter()
            .enumerate()
            .filter_map(|(idx, f)| match FieldSpec::from_field(&f).unwrap() {
                FieldSpec::Content(spec) => Some(match &f.ident {
                    Some(i) => FieldInfo {
                        match_field: (),
                        match_ident: (),
                        expr: quote!(self.#i),
                        spec,
                    },
                    None => FieldInfo {
                        match_field: (),
                        match_ident: (),
                        expr: quote!(self.#idx),
                        spec,
                    },
                }),
                FieldSpec::Ignore => None,
            })
            .collect();
            gen_impl(
                input.ident,
                gen_has_converted(&s),
                gen_convert(&s),
                gen_main_content(&s).0,
            )
        }
        Data::Enum(e) => {
            if e.variants.is_empty() {
                return gen_impl(input.ident, quote!(true), TokenStream::new(), quote!(None));
            }
            let mut has_convert = TokenStream::new();
            let mut convert = TokenStream::new();
            let mut main_content = TokenStream::new();
            for v in e.variants.into_iter() {
                let VariantRecv {
                    ident,
                    complete,
                    fields,
                } = VariantRecv::from_variant(&v).unwrap();
                let matched = {
                    let mut ret = TokenStream::new();
                    for f in fields.iter() {
                        let m = &f.match_field;
                        let i = &f.match_ident;
                        ret.extend(quote!(#m:#i,));
                    }
                    if !complete {
                        ret.extend(quote!(..));
                    }
                    quote!(Self::#ident { #ret } =>)
                };
                has_convert.extend({
                    let expr = gen_has_converted(&fields);
                    quote!(#matched { #expr })
                });
                convert.extend({
                    let expr = gen_convert(&fields);
                    quote!(#matched { #expr })
                });
                main_content.extend({
                    let (e, i) = gen_main_content(&fields);
                    match i {
                        Some(FieldInfo {
                            match_field,
                            match_ident,
                            ..
                        }) => quote!(Self::#ident { #match_field:#match_ident, .. } => #e,),
                        None => quote!(Self::#ident {..} => #e,),
                    }
                });
            }
            gen_impl(
                input.ident,
                quote! {match self { #has_convert }},
                quote! { match self { #convert } },
                quote! {match self {#main_content}},
            )
        }
        Data::Union(_) => panic!("derive for union is not supported"),
    }
}
