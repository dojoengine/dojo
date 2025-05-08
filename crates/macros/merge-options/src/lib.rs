use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Type};

/// Generates a `merge` method for a struct to be compared with another optional struct of the same
 /// type.
 ///
 /// This pattern is mostly used for CLI arguments, where the user can provide a config file
 /// that will be merged with the default values.
 ///
 /// This macro generates a `merge` method that has the following behavior:
 /// - If the other struct is `None`, the field will not be overwritten and the current `Self` value
 ///   will be kept.
 /// - If the other struct is `Some`, the field will be compared with the default value of the
 ///   struct. Every field that is default in `Self` will be overwritten with the value in `other`.
 ///
 /// This has one drawback at the moment, a value that is not the default in the other struct will
 /// always override the current value (if it's the default in `Self`). This may be inconvenient for
 /// some use cases, but it's a limitation of the current approach.
 ///
 /// # Example
 ///
 /// ```
 /// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, MergeOptions)]
 /// pub struct MyOptions {
 ///     pub port: u16,
 ///     pub peers: Vec<String>,
 ///     pub path: Option<String>,
 ///     pub enabled: bool,
 /// }
 ///
 /// impl Default for MyOptions {
 ///     fn default() -> Self {
 ///         Self { port: 8080, peers: vec![], path: None, enabled: false }
 ///     }
 /// }
 ///
 /// // This macro will generate the following code:
 /// impl MyOptions {
 ///     pub fn merge(&mut self, other: Option<&Self>) {
 ///         if let Some(other) = other {
 ///             let default_values = Self::default();
 ///             
 ///             if self.port == default_values.port {
 ///                 self.port = other.port;
 ///             }
 ///
 ///             if self.peers == default_values.peers {
 ///                 self.peers = other.peers;
 ///             }
 ///
 ///             if self.path == default_values.path {
 ///                 self.path = other.path;
 ///             }
 ///
 ///             // Here we can note that if `Self` wants to enforce `false`, it will be overridden by `other` if it's `true`.
 ///             if self.enabled == default_values.enabled {
 ///                 self.enabled = other.enabled;
 ///             }
 ///         }
 ///     }
 /// }
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