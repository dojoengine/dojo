use std::str::FromStr;

use proc_macro2::TokenStream;
use quote::quote;
use strum_macros::{AsRefStr, EnumString};
use syn::parse::Parser;
use syn::ItemFn;

use crate::config::{Configuration, DEFAULT_ERROR_CONFIG};

// Because syn::AttributeArgs does not implement syn::Parse
type AttributeArgs = syn::punctuated::Punctuated<syn::Meta, syn::Token![,]>;

#[derive(EnumString, AsRefStr)]
#[strum(serialize_all = "snake_case")]
enum RunnerArg {
    BlockTime,
    Fee,
    Validation,
    ChainId,
    Accounts,
    DbDir,
    Binary,
    Output,
    Genesis,
}

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
        .parse2(args.into())
        .and_then(|args| build_config(&input, args));

    match config {
        Ok(config) => todo!(),
        Err(e) => token_stream_with_error(parse_knobs(input, true, DEFAULT_ERROR_CONFIG), e),
    }
}

fn build_config(input: &ItemFn, args: AttributeArgs) -> Result<Configuration, syn::Error> {
    if input.sig.asyncness.is_none() {
        let msg = "the `async` keyword is missing from the function declaration";
        return Err(syn::Error::new_spanned(input.sig.fn_token, msg));
    }

    // let mut config = Configuration::new(is_test, rt_multi_thread);
    // let macro_name = config.macro_name();

    for arg in args {
        match arg {
            syn::Meta::NameValue(namevalue) => {
                let ident = namevalue
                    .path
                    .get_ident()
                    .ok_or_else(|| {
                        syn::Error::new_spanned(&namevalue, "Must have specified ident")
                    })?
                    .to_string()
                    .to_lowercase();

                // the value of the attribute
                let lit = match &namevalue.value {
                    syn::Expr::Lit(syn::ExprLit { lit, .. }) => lit,
                    expr => return Err(syn::Error::new_spanned(expr, "Must be a literal")),
                };

                // the ident of the attribute
                let ident = ident.as_str();
                let arg = RunnerArg::from_str(ident);

                match arg {
                    Ok(arg) => match arg {
                        RunnerArg::BlockTime => {
                            //     config.set_worker_threads(lit.clone(),
                            // syn::spanned::Spanned::span(lit))?;
                        }
                        RunnerArg::Fee => {}
                        RunnerArg::Validation => {}
                        RunnerArg::Accounts => {}
                        RunnerArg::ChainId => {}
                        RunnerArg::DbDir => {}
                        RunnerArg::Genesis => {}
                        RunnerArg::Output => {}
                        RunnerArg::Binary => {}
                    },

                    Err(_) => {
                        let msg = format!(
                            "Unknown attribute {ident} is specified; expected one of: `fee`, \
                             `validation`, `accounts`, `chain_id`, `db_dir`, `genesis`, `output`, \
                             binary`",
                        );
                        return Err(syn::Error::new_spanned(namevalue, msg));
                    }
                }
            }

            other => {
                return Err(syn::Error::new_spanned(other, "Unknown attribute inside the macro"));
            }
        }
    }

    // config.build();

    todo!()
}

fn parse_knobs(mut input: ItemFn, is_test: bool, config: Configuration) -> TokenStream {
    todo!()
}

fn token_stream_with_error(mut tokens: TokenStream, error: syn::Error) -> TokenStream {
    tokens.extend(error.into_compile_error());
    tokens
}
