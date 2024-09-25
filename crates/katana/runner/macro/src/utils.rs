use proc_macro2::Span;
use syn::{Attribute, Error, Lit, PathSegment};

pub fn parse_string(int: Lit, span: Span, field: &str) -> Result<String, Error> {
    match int {
        Lit::Str(s) => Ok(s.value()),
        Lit::Verbatim(s) => Ok(s.to_string()),
        _ => Err(Error::new(span, format!("Failed to parse value of `{field}` as string."))),
    }
}

pub fn parse_path(lit: Lit, span: Span, field: &str) -> Result<syn::Path, Error> {
    match lit {
        Lit::Str(s) => {
            let err = Error::new(
                span,
                format!("Failed to parse value of `{field}` as path: \"{}\"", s.value()),
            );
            s.parse::<syn::Path>().map_err(|_| err.clone())
        }
        _ => Err(Error::new(span, format!("Failed to parse value of `{}` as path.", field))),
    }
}

pub fn parse_bool(bool: Lit, span: proc_macro2::Span, field: &str) -> Result<bool, Error> {
    match bool {
        Lit::Bool(b) => Ok(b.value),
        _ => Err(Error::new(span, format!("Failed to parse value of `{field}` as bool."))),
    }
}

pub fn parse_int(int: Lit, span: proc_macro2::Span, field: &str) -> Result<usize, Error> {
    match int {
        Lit::Int(lit) => match lit.base10_parse::<usize>() {
            Ok(value) => Ok(value),
            Err(e) => {
                Err(Error::new(span, format!("Failed to parse value of `{field}` as integer: {e}")))
            }
        },
        _ => Err(Error::new(span, format!("Failed to parse value of `{field}` as integer."))),
    }
}

pub fn attr_ends_with(attr: &Attribute, segment: &PathSegment) -> bool {
    attr.path().segments.iter().last() == Some(segment)
}
