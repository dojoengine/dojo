use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, parse_quote, Stmt};

/// Default runner block interval
const DEFAULT_BLOCK_TIME: u64 = 3000; // 3 seconds

/// Parses the metadata string into the number of accounts and the block time.
///
/// # Arguments
///
/// * `metadata` - The metadata string to parse. The string is expected to be in the format of
///   `n_accounts,executable,block_time` where `block_time` is either a number (time block is ms) or
///   a boolean with `false` to use instand mining, and `true` to use the default block time.
///
/// # Returns
///
/// A tuple containing the number of accounts, the path to the katana program and the block time.
fn parse_metadata(metadata: String) -> (u16, Option<String>, Option<u64>) {
    if metadata.is_empty() {
        return (2, None, None);
    }

    let args = metadata.split(',').collect::<Vec<&str>>();
    let n_accounts = if !args.is_empty() { args[0].parse::<u16>().unwrap() } else { 1 };

    // Block time can be `false` to be `None`, or a number to be `Some(block_time_ms)`.
    // if set to `true`, we use a default block time.
    let block_time = if args.len() >= 2 {
        if let Ok(b) = args[1].trim().parse::<bool>() {
            if !b {
                None
            } else {
                Some(DEFAULT_BLOCK_TIME)
            }
        } else if let Ok(block_time_ms) = args[1].trim().parse::<u64>() {
            Some(block_time_ms)
        } else {
            None
        }
    } else {
        None
    };

    let executable = if args.len() >= 3 {
        args[2].trim()
    } else {
        return (2, None, None);
    };

    let executable = executable.replace('"', "");

    // plus one as the first account is used for deployment
    (n_accounts + 1, Some(executable), block_time)
}

#[proc_macro_attribute]
pub fn katana_test(metadata: TokenStream, input: TokenStream) -> TokenStream {
    let mut test_function = parse_macro_input!(input as syn::ItemFn);
    let function_name = test_function.sig.ident.to_string();

    let (n_accounts, executable, block_time) = parse_metadata(metadata.to_string());

    let block_time = block_time.map(|b| quote!(Some(#b))).unwrap_or(quote!(None));

    let program_name = executable.map(|b| quote!(Some(String::from(#b)))).unwrap_or(quote!(None));

    let header: Stmt = parse_quote! {
        let runner =
            katana_runner::KatanaRunner::new_with_config(
                katana_runner::KatanaRunnerConfig {
                    program_name: #program_name,
                    run_name: Some(String::from(#function_name)),
                    block_time: #block_time,
                    n_accounts: #n_accounts,
                    ..Default::default()
                }
            )
                .expect("failed to start katana");
    };

    test_function.block.stmts.insert(0, header);

    if test_function.sig.asyncness.is_none() {
        TokenStream::from(quote! {
            #[test]
            #test_function
        })
    } else {
        TokenStream::from(quote! {
            #[tokio::test]
            #test_function
        })
    }
}

#[proc_macro] // Needed because the main macro doesn't work with proptest
pub fn runner(metadata: TokenStream) -> TokenStream {
    let metadata = metadata.to_string();
    let mut args = metadata.split(',').collect::<Vec<&str>>();
    let function_name = args.remove(0);

    let (n_accounts, executable, block_time) = parse_metadata(args.join(","));

    let block_time = block_time.map(|b| quote!(Some(#b))).unwrap_or(quote!(None));

    let program_name = executable.map(|b| quote!(Some(String::from(#b)))).unwrap_or(quote!(None));

    TokenStream::from(quote! {
        lazy_static::lazy_static! {
            pub static ref RUNNER: std::sync::Arc<katana_runner::KatanaRunner> = std::sync::Arc::new(
                katana_runner::KatanaRunner::new_with_config(
                    katana_runner::KatanaRunnerConfig {
                        program_name: #program_name,
                        run_name: Some(String::from(#function_name)),
                        block_time: #block_time,
                        n_accounts: #n_accounts,
                        ..Default::default()
                    }
                )
                    .expect("failed to start katana")
            );

        }

        let runner = &RUNNER;
    })
}
