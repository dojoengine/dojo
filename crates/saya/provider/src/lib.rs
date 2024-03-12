//! Saya providers.
//!
//! A provider in Saya is responsible of fetching blocks data
//! and state updates from Katana.
pub mod error;
pub mod provider;
pub mod rpc;

pub use provider::Provider;

pub type ProviderResult<T, E = error::ProviderError> = Result<T, E>;

const LOG_TARGET: &str = "provider";
