use std::collections::HashSet;

use starknet::core::types::contract::AbiEntry;
use starknet_crypto::Felt;

use crate::local::ResourceLocal;
use crate::remote::ResourceRemote;
use crate::{ContractAddress, DojoSelector, ResourceType};

/// The difference between a local and a remote resource.
///
/// The point of view is the local one.
/// Currently, having the remote resources that are not registered by the current project is not
/// supported, since a world can be permissionlessly updated by anyone.
#[derive(Debug)]
pub enum ResourceDiff {
    /// The resource has been created locally, and is not present in the remote world.
    Created(ResourceLocal),
    /// The resource has been updated locally, and is different from the remote world.
    Updated(ResourceLocal, ResourceRemote),
    /// The local resource is in sync with the remote world.
    Synced(ResourceLocal, ResourceRemote),
}

#[derive(Debug)]
pub struct DiffPermissions {
    /// The local permissions.
    pub local: HashSet<PermissionGrantee>,
    /// The remote permissions.
    pub remote: HashSet<PermissionGrantee>,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct PermissionGrantee {
    /// The tag of the resource to grant permissions to.
    /// The tag may not be present if the resource is not managed by the local project.
    pub tag: Option<String>,
    /// The address of the grantee.
    pub address: ContractAddress,
}

impl DiffPermissions {
    /// Returns the permissions that are only present locally.
    pub fn only_local(&self) -> HashSet<PermissionGrantee> {
        self.local.difference(&self.remote).cloned().collect()
    }

    pub fn only_remote(&self) -> HashSet<PermissionGrantee> {
        self.remote.difference(&self.local).cloned().collect()
    }

    pub fn synced(&self) -> HashSet<PermissionGrantee> {
        self.local.intersection(&self.remote).cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.local.is_empty() && self.remote.is_empty()
    }
}

impl ResourceDiff {
    /// Returns the name of the resource.
    pub fn name(&self) -> String {
        match self {
            ResourceDiff::Created(local) => local.name(),
            ResourceDiff::Updated(local, _) => local.name(),
            ResourceDiff::Synced(local, _) => local.name(),
        }
    }

    /// Returns the namespace of the resource.
    pub fn namespace(&self) -> String {
        match self {
            ResourceDiff::Created(local) => local.namespace(),
            ResourceDiff::Updated(local, _) => local.namespace(),
            ResourceDiff::Synced(local, _) => local.namespace(),
        }
    }

    /// Returns the tag of the resource.
    pub fn tag(&self) -> String {
        match self {
            ResourceDiff::Created(local) => local.tag(),
            ResourceDiff::Updated(local, _) => local.tag(),
            ResourceDiff::Synced(local, _) => local.tag(),
        }
    }

    /// Returns the dojo selector of the resource.
    pub fn dojo_selector(&self) -> DojoSelector {
        match self {
            ResourceDiff::Created(local) => local.dojo_selector(),
            ResourceDiff::Updated(local, _) => local.dojo_selector(),
            ResourceDiff::Synced(local, _) => local.dojo_selector(),
        }
    }

    /// Returns the type of the resource.
    pub fn resource_type(&self) -> ResourceType {
        match self {
            ResourceDiff::Created(local) => local.resource_type(),
            ResourceDiff::Updated(local, _) => local.resource_type(),
            ResourceDiff::Synced(local, _) => local.resource_type(),
        }
    }

    /// Returns the current class hash of the resource.
    pub fn current_class_hash(&self) -> Felt {
        match self {
            ResourceDiff::Created(local) => local.class_hash(),
            ResourceDiff::Updated(_, remote) => remote.current_class_hash(),
            ResourceDiff::Synced(_, remote) => remote.current_class_hash(),
        }
    }

    /// Returns the current metadata hash of the resource.
    pub fn metadata_hash(&self) -> Felt {
        match self {
            ResourceDiff::Created(_) => Felt::ZERO,
            ResourceDiff::Updated(_, remote) => remote.metadata_hash(),
            ResourceDiff::Synced(_, remote) => remote.metadata_hash(),
        }
    }

    pub fn abi(&self) -> Vec<AbiEntry> {
        match self {
            ResourceDiff::Created(local) => local.abi(),
            ResourceDiff::Updated(local, _) => local.abi(),
            ResourceDiff::Synced(local, _) => local.abi(),
        }
    }

    pub fn status(&self) -> String {
        let res = match self {
            ResourceDiff::Created(_) => "Created",
            ResourceDiff::Updated(_, _) => "Updated",
            ResourceDiff::Synced(_, _) => "Synced",
        };

        res.to_string()
    }
}
