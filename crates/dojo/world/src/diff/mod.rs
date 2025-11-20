//! Represents the difference between a local and a remote world.
//!
//! The local and remote worlds are consumed to produce the diff, to avoid duplicating the
//! resources.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use compare::ComparableResource;
use dojo_types::naming;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::{BlockId, BlockTag, StarknetError};
use starknet::providers::{Provider, ProviderError};
use starknet_crypto::Felt;
use tracing::trace;

use super::local::{ResourceLocal, WorldLocal};
use super::remote::{ResourceRemote, WorldRemote};
use crate::config::ProfileConfig;
use crate::local::ExternalContractLocal;
use crate::{utils, ContractAddress, DojoSelector, ResourceType};

mod compare;
mod manifest;
mod resource;

pub use manifest::*;
pub use resource::*;

#[derive(Debug)]
pub struct WorldStatusInfo {
    /// The address of the world.
    pub address: Felt,
    /// The hash of the metadata associated to the world.
    pub metadata_hash: Felt,
    /// The class hash of the world.
    pub class_hash: Felt,
    /// The casm class hash of the world.
    pub casm_class_hash: Felt,
    /// The sierra class of the world.
    pub class: SierraClass,
    /// The status of the world.
    pub status: WorldStatus,
    /// The entrypoints of the world.
    pub entrypoints: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub enum WorldStatus {
    /// The world is not deployed, it's the first migration with the given seed.
    NotDeployed,
    /// The local world is a new version, and the remote world must be updated.
    NewVersion,
    /// The world is in sync with the remote world, same dojo version.
    Synced,
}

impl std::fmt::Display for WorldStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorldStatus::NotDeployed => write!(f, "Not Deployed"),
            WorldStatus::NewVersion => write!(f, "To Upgrade"),
            WorldStatus::Synced => write!(f, "Synced"),
        }
    }
}

#[derive(Debug)]
pub struct WorldDiff {
    /// The status of the world.
    pub world_info: WorldStatusInfo,
    /// The namespaces registered in the local world. A list of namespaces is kept
    /// additionally to the resources to ensure they can be processed first,
    /// since the all other resources are namespaced.
    pub namespaces: Vec<DojoSelector>,
    /// The resources registered in the local world, by dojo selector.
    pub resources: HashMap<DojoSelector, ResourceDiff>,
    /// The profile configuration for the world.
    pub profile_config: ProfileConfig,
    /// The external writers.
    pub external_writers: HashMap<DojoSelector, HashSet<ContractAddress>>,
    /// The external owners.
    pub external_owners: HashMap<DojoSelector, HashSet<ContractAddress>>,
}

impl WorldDiff {
    /// Creates a new world diff from a local world.
    ///
    /// Consumes the local world to avoid duplicating the resources.
    pub fn from_local(local: WorldLocal) -> Result<Self> {
        let mut diff = Self {
            world_info: WorldStatusInfo {
                address: local.deterministic_world_address()?,
                metadata_hash: Felt::ZERO,
                class_hash: local.class_hash,
                casm_class_hash: local.casm_class_hash,
                class: local.class,
                status: WorldStatus::NotDeployed,
                entrypoints: local.entrypoints,
            },
            namespaces: vec![],
            resources: HashMap::new(),
            profile_config: local.profile_config,
            external_writers: HashMap::new(),
            external_owners: HashMap::new(),
        };

        for (selector, resource) in local.resources {
            // Namespaces are enumerated to be easily retrieved later.
            if let ResourceLocal::Namespace(_) = &resource {
                diff.namespaces.push(selector);
            }

            diff.resources.insert(selector, ResourceDiff::Created(resource));
        }

        Ok(diff)
    }

    /// Creates a new world diff from a local and a remote world.
    ///
    /// Consumes the local and remote worlds to avoid duplicating the resources,
    /// since the [`ResourceDiff`] will contain one or both of the local and remote resources.
    pub fn new(local: WorldLocal, mut remote: WorldRemote) -> Self {
        let status = if local.class_hash == remote.current_class_hash() {
            WorldStatus::Synced
        } else {
            WorldStatus::NewVersion
        };

        let mut diff = Self {
            world_info: WorldStatusInfo {
                // As the remote world was found, its address is always used.
                address: remote.address,
                metadata_hash: remote.metadata_hash,
                class_hash: local.class_hash,
                casm_class_hash: local.casm_class_hash,
                class: local.class,
                entrypoints: local.entrypoints,
                status,
            },
            namespaces: vec![],
            resources: HashMap::new(),
            profile_config: local.profile_config,
            external_writers: remote.external_writers.clone(),
            external_owners: remote.external_owners.clone(),
        };

        for (local_selector, local_resource) in local.resources {
            // Namespaces are enumerated to be easily retrieved later.
            if let ResourceLocal::Namespace(_) = &local_resource {
                diff.namespaces.push(local_selector);
            }

            let remote_resource = remote.resources.remove(&local_selector);

            if let Some(remote_resource) = remote_resource {
                diff.resources.insert(local_selector, local_resource.compare(remote_resource));
            } else {
                diff.resources.insert(local_selector, ResourceDiff::Created(local_resource));
            }
        }

        diff
    }

    /// Creates a new world diff pulling events from the chain.
    ///
    /// Since node providers are struggling with wide block ranges, we accept a custom
    /// from block to optimize the event fetching.
    pub async fn new_from_chain<P>(
        world_address: Felt,
        world_local: WorldLocal,
        provider: P,
        from_block: Option<u64>,
        max_block_range: u64,
        whitelisted_namespaces: &[String],
    ) -> Result<Self>
    where
        P: Provider,
    {
        let is_deployed =
            match provider.get_class_hash_at(BlockId::Tag(BlockTag::Latest), world_address).await {
                Err(ProviderError::StarknetError(StarknetError::ContractNotFound)) => {
                    trace!(
                        contract_address = format!("{:#066x}", world_address),
                        "World not deployed."
                    );
                    Ok(false)
                }
                Ok(_) => {
                    trace!(
                        contract_address = format!("{:#066x}", world_address),
                        "World already deployed."
                    );
                    Ok(true)
                }
                Err(e) => Err(e),
            }?;

        let namespaces = if whitelisted_namespaces.is_empty() {
            None
        } else {
            Some(whitelisted_namespaces.to_vec())
        };

        if is_deployed {
            let world_remote = WorldRemote::from_events(
                world_address,
                &provider,
                from_block,
                max_block_range,
                namespaces,
            )
            .await?;

            Ok(Self::new(world_local, world_remote))
        } else {
            Self::from_local(world_local)
        }
    }

    /// Returns whether the whole world is in sync.
    ///
    /// This only concerns the resources status, and not the initialization of contracts
    /// or the permissions.
    pub fn is_synced(&self) -> bool {
        self.world_info.status == WorldStatus::Synced
            && self
                .resources
                .values()
                .all(|resource| matches!(resource, ResourceDiff::Synced(_, _)))
    }

    /// Returns the writers of a resource.
    pub fn get_writers(&self, selector: DojoSelector) -> DiffPermissions {
        let resource = self.resources.get(&selector);

        if resource.is_none() {
            return DiffPermissions { local: HashSet::new(), remote: HashSet::new() };
        }

        let resource = resource.unwrap();

        let tag = resource.tag();

        match resource {
            ResourceDiff::Created(_) => {
                let local_writers = self.profile_config.get_local_writers(&tag);
                let local_grantees =
                    local_writers.iter().map(|w| self.resolve_local_grantee(w)).collect();

                DiffPermissions { local: local_grantees, remote: HashSet::new() }
            }
            ResourceDiff::Updated(_, remote) | ResourceDiff::Synced(_, remote) => {
                let local_writers = self.profile_config.get_local_writers(&tag);
                let local_grantees =
                    local_writers.iter().map(|w| self.resolve_local_grantee(w)).collect();

                let remote_writers = remote.get_writers();
                let remote_grantees = remote_writers
                    .1
                    .iter()
                    .map(|addr| self.resolve_remote_grantee(*addr))
                    .collect();

                DiffPermissions { local: local_grantees, remote: remote_grantees }
            }
        }
    }

    /// Returns the owners of a resource.
    pub fn get_owners(&self, selector: DojoSelector) -> DiffPermissions {
        let resource = self.resources.get(&selector);
        if resource.is_none() {
            return DiffPermissions { local: HashSet::new(), remote: HashSet::new() };
        }

        let resource = resource.unwrap();

        let tag = resource.tag();

        match resource {
            ResourceDiff::Created(_) => {
                let local_owners = self.profile_config.get_local_owners(&tag);
                let local_grantees =
                    local_owners.iter().map(|w| self.resolve_local_grantee(w)).collect();

                DiffPermissions { local: local_grantees, remote: HashSet::new() }
            }
            ResourceDiff::Updated(_, remote) | ResourceDiff::Synced(_, remote) => {
                let local_owners = self.profile_config.get_local_owners(&tag);
                let local_grantees =
                    local_owners.iter().map(|w| self.resolve_local_grantee(w)).collect();

                let remote_owners = remote.get_owners();
                let remote_grantees =
                    remote_owners.1.iter().map(|addr| self.resolve_remote_grantee(*addr)).collect();

                DiffPermissions { local: local_grantees, remote: remote_grantees }
            }
        }
    }

    /// Attempts to resolve a local grantee from a tag, to have it's address.
    fn resolve_local_grantee(&self, tag: &str) -> PermissionGrantee {
        let selector = naming::compute_selector_from_tag(tag);

        // TODO: see how we can elegantly have an error from this deep resolve.
        let address = self.get_contract_address(selector).unwrap_or_else(|| {
            panic!(
                "Tag `{}` is not found locally. Local grantee must be managed locally, it's not \
                 supported to manage external resources permissions without a local resource.",
                tag
            )
        });

        PermissionGrantee { tag: Some(tag.to_string()), address }
    }

    /// Attempts to resolve a remote grantee to have it's tag.
    fn resolve_remote_grantee(&self, contract_address: ContractAddress) -> PermissionGrantee {
        let known_addresses = self.get_contracts_addresses();

        let mut tag = None;
        for (selector, address) in &known_addresses {
            if address == &contract_address {
                tag = Some(self.resources.get(selector).unwrap().tag());
                break;
            }
        }

        PermissionGrantee { tag, address: contract_address }
    }

    /// Returns the class of the contract, if any.
    pub fn get_class(&self, selector: DojoSelector) -> Option<&SierraClass> {
        let resource = self.resources.get(&selector)?;

        match resource {
            ResourceDiff::Created(ResourceLocal::Contract(c)) => Some(&c.common.class),
            ResourceDiff::Updated(ResourceLocal::Contract(c), _) => Some(&c.common.class),
            ResourceDiff::Synced(ResourceLocal::Contract(c), _) => Some(&c.common.class),
            _ => None,
        }
    }

    /// Returns the deterministic addresses of the contracts based on the world address.
    pub fn get_contracts_addresses(&self) -> HashMap<DojoSelector, ContractAddress> {
        let mut addresses = HashMap::new();

        for (selector, _) in self.resources.iter() {
            if let Some(address) = self.get_contract_address(*selector) {
                addresses.insert(*selector, address);
            }
        }

        addresses
    }

    /// Returns the deterministic address of a contract from it's tag.
    ///
    /// If the contract is not found or the tag is not valid, returns `None`.
    pub fn get_contract_address_from_tag(&self, tag: &str) -> Option<ContractAddress> {
        self.get_contract_address(naming::compute_selector_from_tag(tag))
    }

    /// Returns the deterministic address of the contract based on the world address.
    pub fn get_contract_address(&self, selector: DojoSelector) -> Option<ContractAddress> {
        let contract_resource = self.resources.get(&selector)?;

        if contract_resource.resource_type() == ResourceType::Contract {
            match contract_resource {
                ResourceDiff::Created(ResourceLocal::Contract(c)) => {
                    Some(utils::compute_dojo_contract_address(
                        selector,
                        c.common.class_hash,
                        self.world_info.address,
                    ))
                }
                ResourceDiff::Updated(_, ResourceRemote::Contract(c)) => Some(c.common.address),
                ResourceDiff::Synced(_, ResourceRemote::Contract(c)) => Some(c.common.address),
                ResourceDiff::Created(ResourceLocal::ExternalContract(c)) => {
                    if let ExternalContractLocal::SozoManaged(c) = c {
                        Some(c.computed_address)
                    } else {
                        None
                    }
                }
                ResourceDiff::Updated(
                    ResourceLocal::ExternalContract(_),
                    ResourceRemote::ExternalContract(r),
                )
                | ResourceDiff::Synced(
                    ResourceLocal::ExternalContract(_),
                    ResourceRemote::ExternalContract(r),
                ) => Some(r.common.address),
                _ => unreachable!(),
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use starknet::core::types::Felt;

    use super::*;
    use crate::config::NamespaceConfig;
    use crate::local::{CommonLocalInfo, ContractLocal, NamespaceLocal, ResourceLocal, WorldLocal};
    use crate::remote::{CommonRemoteInfo, ContractRemote, NamespaceRemote};
    use crate::test_utils::empty_sierra_class;

    #[test]
    fn test_world_diff_new() {
        let ns = "ns".to_string();
        let namespace_config = NamespaceConfig::new(&ns);
        let profile_config = ProfileConfig::new("test", "seed", namespace_config.clone());
        let mut local = WorldLocal::new(profile_config.clone());
        let mut remote = WorldRemote::default();
        // Add one class to the default remote as we simulate it already has some migrations.
        remote.class_hashes.push(Felt::ONE);

        let local_contract = ResourceLocal::Contract(ContractLocal {
            common: CommonLocalInfo {
                name: "c".to_string(),
                namespace: ns.clone(),
                class: empty_sierra_class(),
                casm_class: None,
                class_hash: Felt::ONE,
                casm_class_hash: Felt::ZERO,
            },
            systems: vec![],
        });

        local.add_resource(local_contract.clone());

        let diff = WorldDiff::new(local.clone(), remote.clone());

        assert_eq!(diff.resources.len(), 2);
        assert!(matches!(
            diff.resources.get(&local_contract.dojo_selector()).unwrap(),
            ResourceDiff::Created(_)
        ));

        let remote_contract = ResourceRemote::Contract(ContractRemote {
            common: CommonRemoteInfo::new(Felt::ONE, &ns, "c", Felt::ONE),
            is_initialized: false,
        });

        remote.add_resource(remote_contract.clone());

        let diff = WorldDiff::new(local.clone(), remote.clone());

        assert_eq!(diff.resources.len(), 2);
        assert!(matches!(
            diff.resources.get(&local_contract.dojo_selector()).unwrap(),
            ResourceDiff::Synced(_, _)
        ));

        let mut local = WorldLocal::new(profile_config.clone());

        let local_contract = ResourceLocal::Contract(ContractLocal {
            common: CommonLocalInfo {
                name: "c".to_string(),
                namespace: ns.clone(),
                class: empty_sierra_class(),
                casm_class: None,
                class_hash: Felt::TWO,
                casm_class_hash: Felt::ZERO,
            },
            systems: vec![],
        });

        local.add_resource(local_contract.clone());

        let diff = WorldDiff::new(local.clone(), remote.clone());

        assert_eq!(diff.resources.len(), 2);
        assert!(matches!(
            diff.resources.get(&local_contract.dojo_selector()).unwrap(),
            ResourceDiff::Updated(_, _)
        ));
    }

    #[test]
    fn test_world_diff_namespace() {
        let ns = "ns".to_string();
        let namespace_config = NamespaceConfig::new(&ns);
        let profile_config = ProfileConfig::new("test", "seed", namespace_config.clone());
        let mut local = WorldLocal::new(profile_config.clone());
        let mut remote = WorldRemote::default();
        // Add one class to the default remote as we simulate it already has some migrations.
        remote.class_hashes.push(Felt::ONE);

        let local_namespace =
            ResourceLocal::Namespace(NamespaceLocal { name: "namespace1".to_string() });

        local.add_resource(local_namespace.clone());

        let diff = WorldDiff::new(local.clone(), remote.clone());

        for r in diff.resources.values() {
            println!("{:?} {:?}", r.resource_type(), r.tag());
        }

        assert_eq!(diff.namespaces.len(), 2);
        assert!(matches!(
            diff.resources.get(&naming::compute_bytearray_hash("ns")).unwrap(),
            ResourceDiff::Created(_)
        ));
        assert!(matches!(
            diff.resources.get(&local_namespace.dojo_selector()).unwrap(),
            ResourceDiff::Created(_)
        ));

        let remote_namespace = ResourceRemote::Namespace(NamespaceRemote {
            name: "namespace1".to_string(),
            owners: HashSet::new(),
            writers: HashSet::new(),
        });

        remote.add_resource(remote_namespace.clone());

        let diff = WorldDiff::new(local.clone(), remote.clone());

        assert_eq!(diff.namespaces.len(), 2);
        assert!(matches!(
            diff.resources.get(&naming::compute_bytearray_hash("ns")).unwrap(),
            ResourceDiff::Created(_)
        ));
        assert!(matches!(
            diff.resources.get(&local_namespace.dojo_selector()).unwrap(),
            ResourceDiff::Synced(_, _)
        ));
    }
}
