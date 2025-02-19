//! Attribute macros.
//!
//! An attribute macros is a macro that is used to generate code generally for a struct, enum,
//! module or trait.

pub mod contract;
pub mod event;
pub mod interface;
pub mod library;
pub mod model;

pub use contract::DojoContract;
pub use event::DojoEvent;
pub use interface::DojoInterface;
pub use library::DojoLibrary;
pub use model::DojoModel;

pub const DOJO_CONTRACT_ATTR: &str = "dojo::contract";
pub const DOJO_LIBRARY_ATTR: &str = "dojo::library";
pub const DOJO_INTERFACE_ATTR: &str = "dojo::interface";
pub const DOJO_MODEL_ATTR: &str = "dojo::model";
pub const DOJO_EVENT_ATTR: &str = "dojo::event";
