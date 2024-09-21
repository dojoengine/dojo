use std::str::FromStr;

use crate::parse::{parse_bool, parse_int, parse_path, parse_string};
pub const DEFAULT_ERROR_CONFIG: Configuration = Configuration::new(true);

pub enum RunnerFlavor {
    Embedded,
    Binary,
}

pub struct Configuration {
    dev: bool,
    is_test: bool,
    accounts: Option<u16>,
    fee: Option<bool>,
    validation: Option<bool>,
    db_dir: Option<syn::Path>,
    block_time: Option<u64>,
    log_path: Option<syn::Path>,
    flavor: Option<RunnerFlavor>,
}

impl Configuration {
    pub const fn new(is_test: bool) -> Self {
        Configuration {
            is_test,
            fee: None,
            db_dir: None,
            accounts: None,
            dev: is_test,
            flavor: None,
            log_path: None,
            validation: None,
            block_time: None,
        }
    }

    pub fn set_flavor(
        &mut self,
        flavor: syn::Lit,
        span: proc_macro2::Span,
    ) -> Result<(), syn::Error> {
        if self.flavor.is_some() {
            return Err(syn::Error::new(span, "`flavor` set multiple times."));
        }

        let runner = parse_string(flavor, span, "flavor")?;
        let runner = RunnerFlavor::from_str(&runner).map_err(|err| syn::Error::new(span, err))?;

        self.flavor = Some(runner);
        Ok(())
    }

    pub fn set_db_dir(
        &mut self,
        db_dir: syn::Lit,
        span: proc_macro2::Span,
    ) -> Result<(), syn::Error> {
        if self.db_dir.is_some() {
            return Err(syn::Error::new(span, "`db_dir` set multiple times."));
        }

        let db_dir = parse_path(db_dir, span, "db_dir")?;
        self.db_dir = Some(db_dir);

        Ok(())
    }

    pub fn set_fee(&mut self, fee: syn::Lit, span: proc_macro2::Span) -> Result<(), syn::Error> {
        if self.fee.is_some() {
            return Err(syn::Error::new(span, "`fee` set multiple times."));
        }

        let fee = parse_bool(fee, span, "fee")?;
        self.fee = Some(fee);

        Ok(())
    }

    pub fn set_validation(
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

    pub fn set_block_time(
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

    pub fn set_accounts(
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

impl std::str::FromStr for RunnerFlavor {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "binary" => Ok(RunnerFlavor::Binary),
            "embedded" => Ok(RunnerFlavor::Embedded),
            _ => Err(format!(
                "No such runner flavor `{s}`. The runner flavors are `embedded` and `binary`."
            )),
        }
    }
}
