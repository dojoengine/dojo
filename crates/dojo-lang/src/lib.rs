//! Dojo capabilities and utilities on top of Starknet.
//!
//! Dojo is a full stack toolchain for developing onchain games in Cairo.
//!
//! Learn more at [dojoengine.gg](http://dojoengine.gg).
pub mod compiler;
pub mod contract;
pub mod inline_macros;
pub mod introspect;
mod manifest;
pub mod model;
pub mod plugin;
pub mod print;
pub mod semantics;
pub(crate) mod version;
