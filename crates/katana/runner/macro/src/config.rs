use std::str::FromStr;

use syn::spanned::Spanned;

use crate::parse::{parse_bool, parse_int, parse_path, parse_string};

pub const DEFAULT_ERROR_CONFIG: Configuration = Configuration::new(false);

/// Partial configuration for extracting the individual configuration values from the attribute
/// arguments.
pub struct Configuration {
    pub crate_name: Option<syn::Path>,
    pub dev: bool,
    pub is_test: bool,
    pub accounts: Option<u16>,
    pub fee: Option<bool>,
    pub validation: Option<bool>,
    pub db_dir: Option<String>,
    pub block_time: Option<u64>,
    pub log_path: Option<syn::Path>,
}

impl Configuration {
    const fn new(is_test: bool) -> Self {
        Self {
            is_test,
            fee: None,
            db_dir: None,
            accounts: None,
            dev: is_test,
            log_path: None,
            validation: None,
            block_time: None,
            crate_name: None,
        }
    }

    fn set_crate_name(
        &mut self,
        name: syn::Lit,
        span: proc_macro2::Span,
    ) -> Result<(), syn::Error> {
        if self.crate_name.is_some() {
            return Err(syn::Error::new(span, "`crate` set multiple times."));
        }

        let name_path = parse_path(name, span, "crate")?;
        self.crate_name = Some(name_path);

        Ok(())
    }

    fn set_db_dir(&mut self, db_dir: syn::Lit, span: proc_macro2::Span) -> Result<(), syn::Error> {
        if self.db_dir.is_some() {
            return Err(syn::Error::new(span, "`db_dir` set multiple times."));
        }

        let db_dir = parse_string(db_dir, span, "db_dir")?;
        self.db_dir = Some(db_dir);

        Ok(())
    }

    fn set_fee(&mut self, fee: syn::Lit, span: proc_macro2::Span) -> Result<(), syn::Error> {
        if self.fee.is_some() {
            return Err(syn::Error::new(span, "`fee` set multiple times."));
        }

        let fee = parse_bool(fee, span, "fee")?;
        self.fee = Some(fee);

        Ok(())
    }

    fn set_validation(
        &mut self,
        validation: syn::Lit,
        span: proc_macro2::Span,
    ) -> Result<(), syn::Error> {
        if self.validation.is_some() {
            return Err(syn::Error::new(span, "`validation` set multiple times."));
        }

        let validation = parse_bool(validation, span, "validation")?;
        self.validation = Some(validation);

        Ok(())
    }

    fn set_block_time(
        &mut self,
        block_time: syn::Lit,
        span: proc_macro2::Span,
    ) -> Result<(), syn::Error> {
        if self.block_time.is_some() {
            return Err(syn::Error::new(span, "`block_time` set multiple times."));
        }

        let block_time = parse_int(block_time, span, "block_time")? as u64;
        self.block_time = Some(block_time);

        Ok(())
    }

    fn set_accounts(
        &mut self,
        accounts: syn::Lit,
        span: proc_macro2::Span,
    ) -> Result<(), syn::Error> {
        if self.accounts.is_some() {
            return Err(syn::Error::new(span, "`accounts` set multiple times."));
        }

        let accounts = parse_int(accounts, span, "accounts")? as u16;
        self.accounts = Some(accounts);

        Ok(())
    }
}

enum RunnerArg {
    BlockTime,
    Fee,
    Validation,
    Accounts,
    DbDir,
}

impl std::str::FromStr for RunnerArg {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "block_time" => Ok(RunnerArg::BlockTime),
            "fee" => Ok(RunnerArg::Fee),
            "validation" => Ok(RunnerArg::Validation),
            "accounts" => Ok(RunnerArg::Accounts),
            "db_dir" => Ok(RunnerArg::DbDir),
            _ => Err(format!(
                "Unknown attribute {s} is specified; expected one of: `fee`, `validation`, \
                 `accounts`, `db_dir`, `block_time`",
            )),
        }
    }
}

pub fn build_config(
    input: &crate::parse::ItemFn,
    args: crate::entry::AttributeArgs,
    is_test: bool,
) -> Result<Configuration, syn::Error> {
    // if input.sig.asyncness.is_none() {
    //     let msg = "the `async` keyword is missing from the function declaration";
    //     return Err(syn::Error::new_spanned(input.sig.fn_token, msg));
    // }

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
                let arg = RunnerArg::from_str(ident)
                    .map_err(|err| syn::Error::new_spanned(&namevalue, err))?;

                match arg {
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
                }
            }

            other => {
                return Err(syn::Error::new_spanned(other, "Unknown attribute inside the macro"));
            }
        }
    }

    Ok(config)
}
