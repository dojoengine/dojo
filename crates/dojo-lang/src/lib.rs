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
pub mod system;
pub(crate) mod version;


use crate::compiler::{*};
use crate::component::{*};
use crate::inline_macros::{*};
use crate::manifest::{*};
use crate::plugin::{*};
use crate::print::{*};
use crate::system::{*};
use crate::version::{*};

uniffi_macros::include_scaffolding!("dojo");