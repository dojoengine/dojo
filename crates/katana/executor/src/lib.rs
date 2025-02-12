#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod implementation;
mod utils;

mod abstraction;
pub mod error;
pub use abstraction::*;
