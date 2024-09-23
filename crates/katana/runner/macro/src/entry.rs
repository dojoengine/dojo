use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens};
use syn::parse::Parser;
use syn::{parse_quote, Ident};

use crate::config::{build_config, Configuration, DEFAULT_ERROR_CONFIG};
use crate::item::ItemFn;
use crate::utils::attr_ends_with;

// Because syn::AttributeArgs does not implement syn::Parse
pub type AttributeArgs = syn::punctuated::Punctuated<syn::Meta, syn::Token![,]>;

pub(crate) fn test(args: TokenStream, item: TokenStream) -> TokenStream {
    // If any of the steps for this macro fail, we still want to expand to an item that is as close
    // to the expected output as possible. This helps out IDEs such that completions and other
    // related features keep working.

    let input: ItemFn = match syn::parse2(item.clone()) {
        Ok(it) => it,
        Err(e) => return token_stream_with_error(item, e),
    };

    // parse the attribute arguments
    let config = AttributeArgs::parse_terminated
        .parse2(args)
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

pub fn parse_knobs(input: ItemFn, is_test: bool, config: Configuration) -> TokenStream {
    // If type mismatch occurs, the current rustc points to the last statement.
    let (last_stmt_start_span, last_stmt_end_span) = {
        let mut last_stmt = input.stmts.last().cloned().unwrap_or_default().into_iter();

        // `Span` on stable Rust has a limitation that only points to the first
        // token, not the whole tokens. We can work around this limitation by
        // using the first/last span of the tokens like
        // `syn::Error::new_spanned` does.
        let start = last_stmt.next().map_or_else(Span::call_site, |t| t.span());
        let end = last_stmt.last().map_or(start, |t| t.span());
        (start, end)
    };

    let crate_path = config
        .crate_name
        .map(ToTokens::into_token_stream)
        .unwrap_or_else(|| Ident::new("katana_runner", last_stmt_start_span).into_token_stream());

    let mut cfg: TokenStream = quote! {};

    if let Some(value) = config.block_time {
        cfg = quote_spanned! (last_stmt_start_span=> #cfg block_time: Some(#value), );
    }

    if let Some(value) = config.fee {
        cfg = quote_spanned! (last_stmt_start_span=> #cfg disable_fee: #value, );
    }

    if let Some(value) = config.db_dir {
        cfg = quote_spanned! (last_stmt_start_span=> #cfg db_dir: Some(core::str::FromStr::from_str(#value).expect("valid path")), );
    }

    if let Some(value) = config.accounts {
        cfg = quote_spanned! (last_stmt_start_span=> #cfg n_accounts: #value, );
    }

    if let Some(value) = config.log_path {
        cfg = quote_spanned! (last_stmt_start_span=> #cfg, log_path: Some(#value), );
    }

    if config.dev {
        cfg = quote_spanned! (last_stmt_start_span=> #cfg dev: true, );
    }

    cfg = quote_spanned! {last_stmt_start_span=>
        #crate_path::KatanaRunnerConfig { #cfg ..Default::default() }
    };

    let generated_attrs = if is_test {
        // Don't include the #[test] attribute if it already exists.
        // Otherwise, if we use a proc macro that applies the #[test] attribute (eg.
        // #[tokio::test]), the test would be executed twice.
        if input.attrs().any(|a| attr_ends_with(a, &parse_quote! {test})) {
            quote! {}
        } else {
            quote! {
                #[::core::prelude::v1::test]
            }
        }
    } else {
        quote! {}
    };

    let mut inner = input.clone();
    inner.sig.ident = Ident::new(&format!("___{}", inner.sig.ident), inner.sig.ident.span());
    inner.outer_attrs.clear();
    let inner_name = &inner.sig.ident;

    let last_block = quote_spanned! {last_stmt_end_span=>
        {
            let runner = #crate_path::KatanaRunner::new_with_config(#cfg).expect("Failed to start runner.");
            let ctx = #crate_path::RunnerCtx::new(runner);
            #[allow(clippy::needless_return)]
            return #inner_name(&ctx);
        }
    };
    let inner = quote! { #inner };

    input.into_tokens(generated_attrs, inner, last_block)
}
