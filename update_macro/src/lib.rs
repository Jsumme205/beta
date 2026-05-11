use proc_macro::TokenStream;
use proc_macro2::Span;

use syn::spanned::Spanned;

extern crate proc_macro;

#[proc_macro_derive(Update, attributes(update_field))]
pub fn derive_update(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    let enum_impl = match generate_enum_impl(&input) {
        Ok(enum_impl) => {
            let input = enum_impl.into();
            syn::parse_macro_input!(input as syn::ItemEnum)
        }
        Err(err) => return err.to_compile_error().into(),
    };

    let fn_body = match generate_fn_body(&input, &enum_impl) {
        Ok(body) => body,
        Err(err) => return err.to_compile_error().into(),
    };

    let (impl_gen, ty_gen, where_clause) = input.generics.split_for_impl();
    let ident = &input.ident;
    let enum_ident = &enum_impl.ident;

    quote::quote! {
        #enum_impl

        impl #impl_gen ::update::UpdateField for #ident #ty_gen #where_clause {
            type Update = #enum_ident #ty_gen;

            #fn_body
        }
    }
    .into()
}

fn generate_enum_impl(input: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let enum_name = quote::format_ident!("Update{}", &input.ident);
    let vis = input.vis.clone();
    let generics = &input.generics;

    let fields = match &input.data {
        syn::Data::Enum(_) | syn::Data::Union(_) => {
            return Err(syn::Error::new(input.span(), "expected struct"));
        }
        syn::Data::Struct(s) => &s.fields,
    };

    let iter = fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let ident = ident_fmt(i, field.ident.as_ref());

            let ty: syn::Type = if field
                .attrs
                .iter()
                .any(|v| v.path().is_ident("update_field"))
            {
                let ty = &field.ty;
                syn::parse_quote! {
                    <#ty as ::update::UpdateField>::Update
                }
            } else {
                let ty = &field.ty;
                syn::parse_quote! { #ty }
            };

            EnumVar { ident, ty }
        })
        .collect::<Vec<_>>();

    let id = &input.ident;
    Ok(quote::quote! {
        #vis enum #enum_name #generics {
            __FullUpdate(#id #generics),
            #(#iter),*
        }
    })
}

fn generate_fn_body(
    macro_input: &syn::DeriveInput,
    enum_impl: &syn::ItemEnum,
) -> syn::Result<proc_macro2::TokenStream> {
    let (_, ty_generics, _) = macro_input.generics.split_for_impl();

    let fields = match &macro_input.data {
        syn::Data::Enum(_) | syn::Data::Union(_) => {
            return Err(syn::Error::new(macro_input.span(), "expected struct"));
        }
        syn::Data::Struct(s) => &s.fields,
    };

    let enum_impl_ident = &enum_impl.ident;
    let variants = enum_impl
        .variants
        .iter()
        .skip(1)
        .zip(fields)
        .enumerate()
        .try_fold(
            Vec::new(),
            |mut acc, (index, (variant, field))| -> syn::Result<_> {
                let v = validate_enum_variant(index, variant, field)?;
                acc.push(v);
                Ok(acc)
            },
        )?;

    Ok(quote::quote! {

        fn update(&mut self, val: #enum_impl_ident #ty_generics) {
            use #enum_impl_ident::*;
            match val {
                __FullUpdate(val) => *self = val,
                #(#variants),*
            }
        }
    })
}

struct EnumVar {
    ident: syn::Ident,
    ty: syn::Type,
}

enum Either {
    Ident(syn::Ident),
    Index(syn::Index),
}

struct MatchClause {
    enum_variant: syn::Ident,
    field: Either,
}

impl quote::ToTokens for EnumVar {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { ident, ty } = self;

        let tk = quote::quote! {
            #ident(#ty)
        };
        tokens.extend(tk);
    }
}

impl quote::ToTokens for MatchClause {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            enum_variant,
            field,
        } = self;

        let field = match field {
            Either::Ident(ident) => quote::quote! { #ident},
            Either::Index(idx) => quote::quote! {#idx},
        };

        let tk = quote::quote! {
            #enum_variant(val) => self.#field = val
        };
        tokens.extend(tk);
    }
}

fn ident_fmt(index: usize, ident: Option<&syn::Ident>) -> syn::Ident {
    match ident {
        Some(ident) => syn::Ident::new(
            &format!("Update{}", heck::AsUpperCamelCase(ident.to_string())),
            ident.span(),
        ),
        None => quote::format_ident!("UpdateField{}", index),
    }
}

fn would_create(lhs: &syn::Ident, index: usize, rhs: Option<&syn::Ident>) -> bool {
    ident_fmt(index, rhs).to_string() == lhs.to_string()
}

fn validate_enum_variant(
    index: usize,
    variant: &syn::Variant,
    struct_field: &syn::Field,
) -> syn::Result<MatchClause> {
    if variant.fields.len() != 1 {
        return Err(syn::Error::new(
            Span::call_site(),
            "INTERNAL ERROR: something is seriously broken FIELDS",
        ));
    }

    if !would_create(&variant.ident, index, struct_field.ident.as_ref()) {
        return Err(syn::Error::new(
            struct_field.span(),
            "INTERNAL ERROR: something is seriously broken",
        ));
    }

    let field = match &struct_field.ident {
        Some(ident) => Either::Ident(ident.clone()),
        None => Either::Index(syn::Index {
            span: Span::call_site(),
            index: index as u32,
        }),
    };

    Ok(MatchClause {
        enum_variant: variant.ident.clone(),
        field,
    })
}
