extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn katana_test(_metadata: TokenStream, input: TokenStream) -> TokenStream {
    let mut out = TokenStream::from(quote!(#[tokio::test]));
    out.extend(input);

    out
}
