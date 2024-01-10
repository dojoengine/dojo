extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, parse_quote, Stmt};

#[proc_macro_attribute]
pub fn katana_test(_metadata: TokenStream, input: TokenStream) -> TokenStream {
    let mut test_function = parse_macro_input!(input as syn::ItemFn);
    let function_name = test_function.sig.ident.to_string();

    let header: Stmt = parse_quote! {
        let (__katana_guard, katana_provider) =
            KatanaRunner::new_from_macro(#function_name, 21370).expect("failed to start katana");
    };
    let stmts = &mut test_function.block.stmts;
    stmts.insert(0, header);

    TokenStream::from(quote! {
        #[tokio::test]
        #test_function
    })
}
