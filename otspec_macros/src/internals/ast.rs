//! A Serde ast, parsed from the Syn ast and ready to generate Rust code.

use crate::internals::attr;
use crate::internals::{Ctxt, Derive};
use syn::punctuated::Punctuated;
use syn::Token;

/// A source data structure annotated with `#[derive(Serialize)]` and/or `#[derive(Deserialize)]`,
/// parsed into an internal representation.
pub struct Container<'a> {
    /// The struct or enum name (without generics).
    pub ident: syn::Ident,
    /// Attributes on the structure, parsed for Serde.
    pub attrs: attr::Container,
    /// The contents of the struct or enum.
    pub data: Data<'a>,
    /// Any generics on the struct or enum.
    pub generics: &'a syn::Generics,
    /// Original input.
    pub original: &'a syn::DeriveInput,
}

/// The fields of a struct or enum.
///
/// Analagous to `syn::Data`.
pub enum Data<'a> {
    Enum(Vec<Variant<'a>>),
    Struct(Style, Vec<Field<'a>>),
}

/// A variant of an enum.
pub struct Variant<'a> {
    pub ident: syn::Ident,
    pub attrs: attr::Variant,
    pub style: Style,
    pub fields: Vec<Field<'a>>,
    pub original: &'a syn::Variant,
}

/// A field of a struct.
pub struct Field<'a> {
    pub member: syn::Member,
    pub attrs: attr::Field,
    pub ty: &'a syn::Type,
    pub original: &'a syn::Field,
}

#[derive(Copy, Clone)]
pub enum Style {
    /// Named fields.
    Struct,
    /// Many unnamed fields.
    Tuple,
    /// One unnamed field.
    Newtype,
    /// No fields.
    Unit,
}

impl<'a> Container<'a> {
    /// Convert the raw Syn ast into a parsed container object, collecting errors in `cx`.
    pub fn from_ast(
        cx: &Ctxt,
        item: &'a syn::DeriveInput,
        _derive: Derive,
    ) -> Option<Container<'a>> {
        let attrs = attr::Container::from_ast(cx, item);

        let data = match &item.data {
            syn::Data::Enum(data) => Data::Enum(enum_from_ast(cx, &data.variants)),
            syn::Data::Struct(data) => {
                let (style, fields) = struct_from_ast(cx, &data.fields, None);
                Data::Struct(style, fields)
            }
            syn::Data::Union(_) => {
                cx.error_spanned_by(item, "Serde does not support derive for unions");
                return None;
            }
        };

        let item = Container {
            ident: item.ident.clone(),
            attrs,
            data,
            generics: &item.generics,
            original: item,
        };
        Some(item)
    }
}

fn enum_from_ast<'a>(
    cx: &Ctxt,
    variants: &'a Punctuated<syn::Variant, Token![,]>,
) -> Vec<Variant<'a>> {
    variants
        .iter()
        .map(|variant| {
            let attrs = attr::Variant::from_ast(cx, variant);
            let (style, fields) = struct_from_ast(cx, &variant.fields, Some(&attrs));
            Variant {
                ident: variant.ident.clone(),
                attrs,
                style,
                fields,
                original: variant,
            }
        })
        .collect()
}

fn struct_from_ast<'a>(
    cx: &Ctxt,
    fields: &'a syn::Fields,
    attrs: Option<&attr::Variant>,
) -> (Style, Vec<Field<'a>>) {
    match fields {
        syn::Fields::Named(fields) => (Style::Struct, fields_from_ast(cx, &fields.named, attrs)),
        syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            (Style::Newtype, fields_from_ast(cx, &fields.unnamed, attrs))
        }
        syn::Fields::Unnamed(fields) => (Style::Tuple, fields_from_ast(cx, &fields.unnamed, attrs)),
        syn::Fields::Unit => (Style::Unit, Vec::new()),
    }
}

fn fields_from_ast<'a>(
    cx: &Ctxt,
    fields: &'a Punctuated<syn::Field, Token![,]>,
    attrs: Option<&attr::Variant>,
) -> Vec<Field<'a>> {
    fields
        .iter()
        .enumerate()
        .map(|(i, field)| Field {
            member: match &field.ident {
                Some(ident) => syn::Member::Named(ident.clone()),
                None => syn::Member::Unnamed(i.into()),
            },
            attrs: attr::Field::from_ast(cx, i, field, attrs),
            ty: &field.ty,
            original: field,
        })
        .collect()
}
