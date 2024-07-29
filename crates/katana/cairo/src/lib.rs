#![warn(unused_crate_dependencies)]

//! Re-export of the Cairo language crates used throughout Katana.

pub mod lang {
    pub extern crate cairo_lang_casm as casm;
    pub extern crate cairo_lang_runner as runner;
    pub extern crate cairo_lang_sierra as sierra;
    pub extern crate cairo_lang_sierra_to_casm as sierra_to_casm;
    pub extern crate cairo_lang_starknet as starknet;
    pub extern crate cairo_lang_starknet_classes as starknet_classes;
    pub extern crate cairo_lang_utils as utils;
}

pub use {cairo_vm, starknet_api};
