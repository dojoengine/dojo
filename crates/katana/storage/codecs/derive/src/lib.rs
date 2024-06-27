use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_attribute]
#[rustfmt::skip]
#[allow(unreachable_code)]
pub fn main_codec(args: TokenStream, input: TokenStream) -> TokenStream {
    #[cfg(feature = "postcard")]
    return use_postcard(args, input);

    no_codec(args, input)
}

#[proc_macro_attribute]
pub fn no_codec(_args: TokenStream, input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    quote! { #ast }.into()
}

#[proc_macro_attribute]
pub fn use_postcard(_args: TokenStream, input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    quote! {
        #[derive(serde::Serialize, serde::Deserialize)]
        #ast
    }
    .into()
}
