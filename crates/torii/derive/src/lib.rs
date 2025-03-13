use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

/// Generates a `merge` method for option structs.
/// 
/// This macro automatically generates a merge function that follows the pattern
/// seen in the IndexingOptions struct, where fields are only overwritten from
/// the other struct if they have default values.
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
///         Self {
///             port: 8080,
///             peers: vec![],
///             path: None,
///             enabled: false,
///         }
///     }
/// }
/// ```
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
        
        // For all field types, check against the default value
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