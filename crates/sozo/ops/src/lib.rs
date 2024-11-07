// #![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod migrate;
pub mod migration_ui;
pub mod model;
pub mod resource_descriptor;

#[cfg(test)]
pub mod tests;
