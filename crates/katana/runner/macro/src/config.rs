#![allow(unused)]

use std::str::FromStr;

use syn::spanned::Spanned;

use crate::utils::{parse_bool, parse_int, parse_path, parse_string};

pub const DEFAULT_ERROR_CONFIG: Configuration = Configuration::new(false);

pub struct Configuration {
    pub crate_name: Option<syn::Path>,
    pub dev: bool,
    pub is_test: bool,
    pub accounts: Option<syn::Expr>,
    pub fee: Option<syn::Expr>,
    pub validation: Option<syn::Expr>,
    pub db_dir: Option<syn::Expr>,
    pub block_time: Option<syn::Expr>,
    pub log_path: Option<syn::Expr>,
    pub chain_id: Option<syn::Expr>,
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
            chain_id: None,
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

    fn set_db_dir(&mut self, db_dir: syn::Expr, span: proc_macro2::Span) -> Result<(), syn::Error> {
        if self.db_dir.is_some() {
            return Err(syn::Error::new(span, "`db_dir` set multiple times."));
        }

        self.db_dir = Some(db_dir);
        Ok(())
    }

    fn set_fee(&mut self, fee: syn::Expr, span: proc_macro2::Span) -> Result<(), syn::Error> {
        if self.fee.is_some() {
            return Err(syn::Error::new(span, "`fee` set multiple times."));
        }

        self.fee = Some(fee);
        Ok(())
    }

    fn set_validation(
        &mut self,
        validation: syn::Expr,
        span: proc_macro2::Span,
    ) -> Result<(), syn::Error> {
        if self.validation.is_some() {
            return Err(syn::Error::new(span, "`validation` set multiple times."));
        }

        self.validation = Some(validation);
        Ok(())
    }

    fn set_block_time(
        &mut self,
        block_time: syn::Expr,
        span: proc_macro2::Span,
    ) -> Result<(), syn::Error> {
        if self.block_time.is_some() {
            return Err(syn::Error::new(span, "`block_time` set multiple times."));
        }

        self.block_time = Some(block_time);
        Ok(())
    }

    fn set_accounts(
        &mut self,
        accounts: syn::Expr,
        span: proc_macro2::Span,
    ) -> Result<(), syn::Error> {
        if self.accounts.is_some() {
            return Err(syn::Error::new(span, "`accounts` set multiple times."));
        }

        self.accounts = Some(accounts);
        Ok(())
    }

    fn set_chain_id(
        &mut self,
        chain_id: syn::Expr,
        span: proc_macro2::Span,
    ) -> Result<(), syn::Error> {
        if self.chain_id.is_some() {
            return Err(syn::Error::new(span, "`chain_id` set multiple times."));
        }

        self.chain_id = Some(chain_id);
        Ok(())
    }
}

enum RunnerArg {
    BlockTime,
    Fee,
    Validation,
    Accounts,
    DbDir,
    ChainId,
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
            "chain_id" => Ok(RunnerArg::ChainId),
            _ => Err(format!(
                "Unknown attribute {s} is specified; expected one of: `fee`, `validation`, \
                 `accounts`, `db_dir`, `block_time`, `chain_id`",
            )),
        }
    }
}

pub fn build_config(
    _input: &crate::item::ItemFn,
    args: crate::entry::AttributeArgs,
    is_test: bool,
) -> Result<Configuration, syn::Error> {
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

                // the ident of the attribute
                let ident = ident.as_str();
                let arg = RunnerArg::from_str(ident)
                    .map_err(|err| syn::Error::new_spanned(&namevalue, err))?;

                let expr = &namevalue.value;

                match arg {
                    RunnerArg::BlockTime => {
                        config.set_block_time(expr.clone(), Spanned::span(&namevalue))?
                    }
                    RunnerArg::Validation => {
                        config.set_validation(expr.clone(), Spanned::span(&namevalue))?
                    }
                    RunnerArg::Accounts => {
                        config.set_accounts(expr.clone(), Spanned::span(&namevalue))?;
                    }
                    RunnerArg::DbDir => {
                        config.set_db_dir(expr.clone(), Spanned::span(&namevalue))?
                    }
                    RunnerArg::ChainId => {
                        config.set_chain_id(expr.clone(), Spanned::span(&namevalue))?
                    }
                    RunnerArg::Fee => config.set_fee(expr.clone(), Spanned::span(&namevalue))?,
                }
            }

            other => {
                return Err(syn::Error::new_spanned(other, "Unknown attribute inside the macro"));
            }
        }
    }

    Ok(config)
}
