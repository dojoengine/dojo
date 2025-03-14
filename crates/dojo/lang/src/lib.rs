//! Dojo compiler.
//!
//! This crate contains the Dojo compiler, with a cairo plugin for the Cairo language.

#![warn(unused_crate_dependencies)]

pub mod attribute_macros;
pub mod aux_data;
pub mod cairo_plugin;
pub mod derive_macros;
pub mod inline_macros;
pub mod semantics;
pub mod syntax;
pub mod utils;

pub use cairo_plugin::{dojo_plugin_suite, BuiltinDojoPlugin, DOJO_PLUGIN_PACKAGE_NAME};

pub const CAIRO_PATH_SEPARATOR: &str = "::";
pub const WORLD_QUALIFIED_PATH: &str = "dojo::world::world_contract::world";
pub const WORLD_CONTRACT_TAG: &str = "dojo-world";
pub const RESOURCE_METADATA_QUALIFIED_PATH: &str = "dojo::model::metadata::resource_metadata";
pub const CONTRACTS_DIR: &str = "contracts";
pub const MODELS_DIR: &str = "models";
pub const EVENTS_DIR: &str = "events";
pub const MANIFESTS_DIR: &str = "manifests";
pub const MANIFESTS_BASE_DIR: &str = "base";

/// Prints the given string only if the `DOJO_EXPAND` environemnt variable is set.
/// This is useful for debugging the compiler with verbose output.
///
/// # Arguments
///
/// * `loc` - The location of the code to be expanded.
/// * `code` - The code to be expanded.
pub fn debug_expand(loc: &str, code: &str) {
    if std::env::var("DOJO_EXPAND").is_ok() {
        println!(
            "\n*> EXPAND {} <*\n>>>>>>>>>>>>>>>>>>>>>>>>>>>\n{}\n<<<<<<<<<<<<<<<<<<<<<<<<<<<\n",
            loc, code
        );
    }
}

/// Prints the given string only if the 'DOJO_STORE_EXPAND' environment variable is set.
/// This is useful for debugging DojoStore implementation.
pub fn debug_store_expand(element_name: &str, code: &str) {
    if std::env::var("DOJO_STORE_EXPAND").is_ok() {
        println!(
            "\n*> EXPAND {} <*\n>>>>>>>>>>>>>>>>>>>>>>>>>>>\n{}\n<<<<<<<<<<<<<<<<<<<<<<<<<<<\n",
            element_name, code
        );
    }
}
