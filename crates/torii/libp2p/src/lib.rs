#![warn(unused_crate_dependencies)]

pub mod client;
mod constants;
pub mod errors;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;
mod tests;
pub mod typed_data;
pub mod types;
