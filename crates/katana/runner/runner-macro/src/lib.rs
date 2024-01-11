extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, parse_quote, Stmt};

#[proc_macro_attribute]
pub fn katana_test(metadata: TokenStream, input: TokenStream) -> TokenStream {
    let mut test_function = parse_macro_input!(input as syn::ItemFn);
    let function_name = test_function.sig.ident.to_string();

    let metadata = metadata.to_string();

    let n_accounts = if metadata.len() != 0 { metadata.parse::<u16>().unwrap() } else { 1 };

    let header: Stmt = parse_quote! {
        let runner =
            katana_runner::KatanaRunner::new_with_name_and_accounts(#function_name, #n_accounts)
                .expect("failed to start katana");
    };

    test_function.block.stmts.insert(0, header);

    TokenStream::from(quote! {
        #[tokio::test]
        #test_function
    })
}
