//! # Walnut Integration Module
//!
//! This module integrates Walnut, a debugger for Starknet transactions, with Dojo,
//! enhancing Dojo's capabilities by allowing users to debug transactions.
//!
//! The integration introduces a `--walnut` flag to the `sozo migrate apply` and `sozo execute`
//! commands.
//!
//! Using the --walnut flag with the `sozo migrate apply` command performs a verification process,
//! during which the source code of the Dojo project is uploaded and stored on Walnut.
//! The source code of each class will be linked with the respective class hash.
//!
//! When running the `sozo execute` command with the `--walnut` flag, a link to the Walnut debugger
//! page is printed to the terminal, allowing users to debug their transactions.
//!
//! Note:
//! - Classes should be verified with `sozo migrate apply --walnut` before debugging transactions.
//! - This feature is only supported on hosted networks.

pub mod debugger;
pub mod transaction;
pub mod utils;
pub mod verification;

pub use debugger::WalnutDebugger;

pub const WALNUT_APP_URL: &str = "https://app.walnut.dev";
pub const WALNUT_API_URL: &str = "https://api.walnut.dev";
pub const WALNUT_API_KEY_ENV_VAR: &str = "WALNUT_API_KEY";
pub const WALNUT_API_URL_ENV_VAR: &str = "WALNUT_API_URL";
