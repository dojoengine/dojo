use std::str::FromStr;

use proc_macro2::TokenStream;
use strum_macros::{AsRefStr, EnumString};
use syn::parse::Parser;
use syn::spanned::Spanned;

use crate::config::{Configuration, DEFAULT_ERROR_CONFIG};
use crate::parse::parse_knobs;

// Because syn::AttributeArgs does not implement syn::Parse
type AttributeArgs = syn::punctuated::Punctuated<syn::Meta, syn::Token![,]>;

#[derive(EnumString, AsRefStr)]
#[strum(serialize_all = "snake_case")]
enum RunnerArg {
    BlockTime,
    Fee,
    Validation,
    Accounts,
    DbDir,
    Flavor,
}

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

fn build_config(
    input: &crate::parse::ItemFn,
    args: AttributeArgs,
    is_test: bool,
) -> Result<Configuration, syn::Error> {
    if input.sig.asyncness.is_none() {
        let msg = "the `async` keyword is missing from the function declaration";
        return Err(syn::Error::new_spanned(input.sig.fn_token, msg));
    }

    let mut config = Configuration::new(is_test);

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
                        RunnerArg::Flavor => {
                            config.set_flavor(lit.clone(), Spanned::span(lit))?;
                        }
                        RunnerArg::BlockTime => {
                            config.set_block_time(lit.clone(), Spanned::span(lit))?
                        }
                        RunnerArg::Validation => {
                            config.set_validation(lit.clone(), Spanned::span(lit))?
                        }
                        RunnerArg::Accounts => {
                            config.set_accounts(lit.clone(), Spanned::span(lit))?;
                        }

                        RunnerArg::Fee => config.set_fee(lit.clone(), Spanned::span(lit))?,
                        RunnerArg::DbDir => config.set_db_dir(lit.clone(), Spanned::span(lit))?,
                    },

                    Err(_) => {
                        let msg = format!(
                            "Unknown attribute {ident} is specified; expected one of: `flavor`, \
                             `fee`, `validation`, `accounts`, `db_dir`, `block_time`",
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

    Ok(config)
}

fn token_stream_with_error(mut tokens: TokenStream, error: syn::Error) -> TokenStream {
    tokens.extend(error.into_compile_error());
    tokens
}
