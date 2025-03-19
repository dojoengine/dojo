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

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("MergeOptions can only be derived for structs with named fields"),
        },
        _ => panic!("MergeOptions can only be derived for structs"),
    };

    let merge_fields = fields.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();

        // For all field types, check against the default value.
        quote! {
            if self.#field_name == default_values.#field_name {
                self.#field_name = other.#field_name.clone();
            }
        }
    });

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
