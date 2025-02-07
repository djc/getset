use crate::attr_name;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::{Ident, Span};
use syn::{Attribute, Field, Lit, Meta, MetaNameValue};

pub struct GenParams {
    pub attribute_name: &'static str,
    pub fn_name_prefix: &'static str,
    pub fn_name_suffix: &'static str,
    pub global_attr: Option<Meta>,
}

#[derive(PartialEq, Eq)]
pub enum GenMode {
    Get,
    Set,
    GetMut,
}

pub fn attr_tuple(attr: &Attribute) -> Option<(Ident, Meta)> {
    let meta = attr.interpret_meta();
    meta.map(|v| (v.name(), v))
}

pub fn parse_visibility(attr: Option<&Meta>, meta_name: &str) -> Option<Ident> {
    match attr {
        // `#[get = "pub"]` or `#[set = "pub"]`
        Some(Meta::NameValue(MetaNameValue {
            lit: Lit::Str(ref s),
            ident,
            ..
        })) => {
            if ident == meta_name {
                s.value()
                    .split(' ')
                    .find(|v| *v != "with_prefix")
                    .map(|v| Ident::new(&v, Span::call_site()))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Some users want legacy/compatability.
/// (Getters are often prefixed with `get_`)
fn has_prefix_attr(f: &Field) -> bool {
    let inner = f
        .attrs
        .iter()
        .filter(|v| attr_name(v).expect("Could not get attribute") == "get")
        .last()
        .and_then(|v| v.parse_meta().ok());
    match inner {
        Some(Meta::NameValue(meta)) => {
            if let Lit::Str(lit) = meta.lit {
                // Naive tokenization to avoid a possible visibility mod named `with_prefix`.
                lit.value().split(' ').any(|v| v == "with_prefix")
            } else {
                false
            }
        }
        _ => false,
    }
}

pub fn implement(field: &Field, mode: &GenMode, params: &GenParams) -> TokenStream2 {
    let field_name = field
        .clone()
        .ident
        .expect("Expected the field to have a name");

    let fn_name = Ident::new(
        &format!(
            "{}{}{}{}",
            if has_prefix_attr(field) && (*mode == GenMode::Get || *mode == GenMode::GetMut) {
                "get_"
            } else {
                ""
            },
            params.fn_name_prefix,
            field_name,
            params.fn_name_suffix
        ),
        Span::call_site(),
    );
    let ty = field.ty.clone();

    let mut doc = Vec::new();
    let attr = field
        .attrs
        .iter()
        .filter_map(|v| {
            let tuple = attr_tuple(v).expect("attribute");
            match tuple.0.to_string().as_str() {
                "doc" => {
                    doc.push(v);
                    None
                }
                name if params.attribute_name == name => Some(tuple.1),
                _ => None,
            }
        })
        .last()
        .or_else(|| params.global_attr.clone());

    let visibility = parse_visibility(attr.as_ref(), params.attribute_name);
    match attr {
        Some(_) => match mode {
            GenMode::Get => {
                quote! {
                    #(#doc)*
                    #[inline(always)]
                    #visibility fn #fn_name(&self) -> &#ty {
                        &self.#field_name
                    }
                }
            }
            GenMode::Set => {
                quote! {
                    #(#doc)*
                    #[inline(always)]
                    #visibility fn #fn_name(&mut self, val: #ty) -> &mut Self {
                        self.#field_name = val;
                        self
                    }
                }
            }
            GenMode::GetMut => {
                quote! {
                    #(#doc)*
                    #[inline(always)]
                    #visibility fn #fn_name(&mut self) -> &mut #ty {
                        &mut self.#field_name
                    }
                }
            }
        },
        // Don't need to do anything.
        None => quote! {},
    }
}
