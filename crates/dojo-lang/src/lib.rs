//! Dojo capabilities and utilities on top of Starknet.
//!
//! Dojo is a full stack toolchain for developing onchain games in Cairo.
//!
//! Learn more at [dojoengine.gg](http://dojoengine.gg).
pub mod compiler;
pub mod contract;
pub mod inline_macros;
pub mod introspect;
pub mod model;
pub mod plugin;
pub mod print;
pub mod semantics;
pub(crate) mod version;

// Copy of non pub functions from scarb + extension.
// Also used by `sozo`.
pub mod scarb_internal;
