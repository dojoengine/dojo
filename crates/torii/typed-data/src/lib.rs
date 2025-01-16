#![warn(unused_crate_dependencies)]

#[cfg(test)]
mod test;

pub mod error;
pub mod typed_data;

pub use typed_data::TypedData;
