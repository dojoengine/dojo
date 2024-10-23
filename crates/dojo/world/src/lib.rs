#[cfg(feature = "metadata")]
pub mod config;
#[cfg(feature = "manifest")]
pub mod manifest;
#[cfg(feature = "metadata")]
pub mod metadata;
#[cfg(feature = "migration")]
pub mod migration;
#[cfg(feature = "metadata")]
pub mod uri;

pub mod local;
pub mod remote;
pub mod contracts;
