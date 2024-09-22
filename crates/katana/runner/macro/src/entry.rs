use proc_macro2::TokenStream;
use syn::parse::Parser;

use crate::config::{build_config, DEFAULT_ERROR_CONFIG};
use crate::parse::parse_knobs;

// Because syn::AttributeArgs does not implement syn::Parse
pub type AttributeArgs = syn::punctuated::Punctuated<syn::Meta, syn::Token![,]>;

pub(crate) fn test(args: TokenStream, item: TokenStream) -> TokenStream {
    // If any of the steps for this macro fail, we still want to expand to an item that is as close
    // to the expected output as possible. This helps out IDEs such that completions and other
    // related features keep working.

    let input: crate::parse::ItemFn = match syn::parse2(item.clone()) {
        Ok(it) => it,
        Err(e) => return token_stream_with_error(item, e),
    };

    // parse the attribute arguments
    let config = AttributeArgs::parse_terminated
        .parse2(args.into())
        .and_then(|args| build_config(&input, args, true));

    match config {
        Ok(config) => parse_knobs(input, true, config),
        Err(e) => token_stream_with_error(parse_knobs(input, false, DEFAULT_ERROR_CONFIG), e),
    }
}

fn token_stream_with_error(mut tokens: TokenStream, error: syn::Error) -> TokenStream {
    tokens.extend(error.into_compile_error());
    tokens
}
