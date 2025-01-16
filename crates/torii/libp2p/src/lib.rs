#![warn(unused_crate_dependencies)]

mod constants;
pub mod error;

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
pub mod server;

#[cfg(test)]
mod test;

pub mod types;
