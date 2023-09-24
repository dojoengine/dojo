//! Dojo capabilities and utilities on top of Starknet.
//!
//! Dojo is a full stack toolchain for developing onchain games in Cairo.
//!
//! Learn more at [dojoengine.gg](http://dojoengine.gg).
pub mod compiler;
pub mod component;
pub mod inline_macros;
mod manifest;
pub mod plugin;
pub mod print;
pub mod schema;
pub mod system;
pub(crate) mod version;
