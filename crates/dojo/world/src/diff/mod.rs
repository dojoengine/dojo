//! Represents the difference between a local and a remote world.
//!

use super::local::{WorldLocal, LocalResource};
use super::remote::{WorldRemote, RemoteResource};

/// The difference between a local and a remote world.
#[derive(Debug)]
pub enum WorldDiff {
    /// The resource has been created locally, and is not present in the remote world.
    Created(LocalResource),
    /// The resource has been updated locally, and is different from the remote world.
    Outdated(LocalResource, RemoteResource),
    /// The local resource is in sync with the remote world.
    Synced(RemoteResource),
    // Last possibility is only found remotely, but this is not supported currently since we are not supposed to be concerned with the resources that are not registered by the current project.
}

impl WorldDiff {
    
}
