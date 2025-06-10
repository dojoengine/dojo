use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

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
/// Fields marked with `#[merge]` will recursively call their own `merge` method instead of being
/// replaced entirely.
///
/// # Example
///
/// ```
/// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, MergeOptions)]
/// pub struct MyOptions {
///     pub port: u16,
///     pub peers: Vec<String>,
///     pub path: Option<String>,
///     #[merge]
///     pub nested: NestedOptions,
///     pub enabled: bool,
/// }
///
/// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, MergeOptions)]
/// pub struct NestedOptions {
///     pub value: String,
/// }
///
/// impl Default for MyOptions {
///     fn default() -> Self {
///         Self {
///             port: 8080,
///             peers: vec![],
///             path: None,
///             nested: NestedOptions::default(),
///             enabled: false,
///         }
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
///                 self.peers = other.peers.clone();
///             }
///
///             if self.path == default_values.path {
///                 self.path = other.path.clone();
///             }
///
///             // The #[merge] attribute causes this field to be merged recursively
///             self.nested.merge(Some(&other.nested));
///
///             if self.enabled == default_values.enabled {
///                 self.enabled = other.enabled;
///             }
///         }
///     }
/// }
#[proc_macro_derive(MergeOptions, attributes(merge))]
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

        // Check if the field has the #[merge] attribute
        let has_merge_attr = field.attrs.iter().any(|attr| attr.path().is_ident("merge"));

        if has_merge_attr {
            // For fields with #[merge] attribute, use recursive merging
            quote! {
                self.#field_name.merge(Some(&other.#field_name));
            }
        } else {
            // For fields without #[merge] attribute, use the default comparison logic
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
