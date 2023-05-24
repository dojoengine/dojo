//! Dojo client library for Bevy game engine.
//!
//! ### What is Bevy?
//!
//! > Bevy is a refreshingly simple data-driven game engine built in Rust. It is free and
//! > open-source forever!
//!
//! ### Useful resources about Bevy
//!
//! - 💻 [GitHub](https://github.com/bevyengine/bevy)
//! - 📖 [Docs.rs](https://docs.rs/bevy/latest/bevy/)
//! - 💡 [Examples](https://github.com/bevyengine/bevy/tree/main/examples)
//! - 🌐 [Website](https://bevyengine.org/)
//! - 📝 [Unofficial Bevy Cheat Book](https://bevy-cheatbook.github.io/)

#[cfg(not(target_arch = "wasm32"))]
pub mod indexer;
pub mod light_client;

/// Sets of tools to bootstrap your `bevy_dojo` project.
///
/// ### Usage
///
/// ```rust, no_run
/// use bevy::prelude::*;
/// use bevy_dojo::prelude::*;
///
/// fn main() {
///     App::new().add_plugins(DefaultPlugins).add_plugin(LightClientPlugin).run();
/// }
/// ```
pub mod prelude {
    #[cfg(not(target_arch = "wasm32"))]
    pub use crate::indexer::IndexerPlugin;
    pub use crate::light_client::prelude::*;
}
