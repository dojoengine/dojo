use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Type};

#[proc_macro_derive(MergeOptions)]
pub fn derive_merge_options(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Ensure the input is a struct with named fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("MergeOptions can only be derived for structs with named fields"),
        },
        _ => panic!("MergeOptions can only be derived for structs"),
    };

    // Generate merge logic for each field
    let merge_fields = fields.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;

        // Heuristic to determine if the field is a struct-like type
        let is_struct_like = match field_type {
            Type::Path(type_path) => {
                let path = &type_path.path;
                // Check if the type is a user-defined type (not in std or core)
                path.segments.iter().all(|seg| {
                    !seg.ident.to_string().starts_with("std")
                        && !seg.ident.to_string().starts_with("core")
                        && !matches!(
                            seg.ident.to_string().as_str(),
                            "i8" | "i16" | "i32" | "i64" | "i128"
                                | "u8" | "u16" | "u32" | "u64" | "u128"
                                | "f32" | "f64"
                                | "bool" | "char" | "str" | "String"
                                | "Vec" | "Option" | "Box" | "Rc" | "Arc"
                        )
                })
            }
            _ => false, // Non-path types (e.g., references, tuples) are treated as non-struct-like
        };

        if is_struct_like {
            // For struct-like types, assume they have a merge method
            quote! {
                self.#field_name.merge(Some(&other.#field_name));
            }
        } else {
            // For non-struct-like types, use the default comparison logic
            quote! {
                if self.#field_name == default_values.#field_name {
                    self.#field_name = other.#field_name.clone();
                }
            }
        }
    });

    // Generate the impl block
    let expanded = quote! {
        impl #name {
            pub fn merge(&mut self, other: Option<&Self>) {
                if let Some(other) = other {
                    let default_values = Self::default();
                    #(#merge_fields)*
                }
            }
        }
    };

    TokenStream::from(expanded)
}