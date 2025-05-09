//! Implements the comparison between a local and a remote resource/world.
//!
//! The point of view is the local one.

use super::ResourceDiff;
use crate::local::{
    ContractLocal, EventLocal, ExternalContractLocal, LibraryLocal, ModelLocal, NamespaceLocal,
    ResourceLocal,
};
use crate::remote::ResourceRemote;

/// A trait to compare a local resource with a remote one.
pub trait ComparableResource {
    /// Compares a local resource with a remote one.
    ///
    /// Takes ownership since the [`ResourceDiff`] will contain one or both resources.
    fn compare(self, remote: ResourceRemote) -> ResourceDiff;
}

impl ComparableResource for ContractLocal {
    fn compare(self, remote: ResourceRemote) -> ResourceDiff {
        let remote_contract = remote.as_contract_or_panic();

        if self.common.class_hash == remote_contract.common.current_class_hash() {
            ResourceDiff::Synced(ResourceLocal::Contract(self), remote)
        } else {
            ResourceDiff::Updated(ResourceLocal::Contract(self), remote)
        }
    }
}

impl ComparableResource for ExternalContractLocal {
    fn compare(self, remote: ResourceRemote) -> ResourceDiff {
        let remote_contract = remote.as_external_contract_or_panic();

        if self.common.class_hash == remote_contract.common.current_class_hash() {
            ResourceDiff::Synced(ResourceLocal::ExternalContract(self), remote)
        } else {
            ResourceDiff::Updated(ResourceLocal::ExternalContract(self), remote)
        }
    }
}

impl ComparableResource for LibraryLocal {
    fn compare(self, remote: ResourceRemote) -> ResourceDiff {
        let remote_contract = remote.as_library_or_panic();

        if self.common.class_hash == remote_contract.common.current_class_hash()
            && self.version == remote_contract.version
        {
            ResourceDiff::Synced(ResourceLocal::Library(self), remote)
        } else {
            ResourceDiff::Created(ResourceLocal::Library(self))
        }
    }
}

impl ComparableResource for ModelLocal {
    fn compare(self, remote: ResourceRemote) -> ResourceDiff {
        let remote_model = remote.as_model_or_panic();

        if self.common.class_hash == remote_model.common.current_class_hash() {
            ResourceDiff::Synced(ResourceLocal::Model(self), remote)
        } else {
            ResourceDiff::Updated(ResourceLocal::Model(self), remote)
        }
    }
}

impl ComparableResource for EventLocal {
    fn compare(self, remote: ResourceRemote) -> ResourceDiff {
        let remote_event = remote.as_event_or_panic();

        if self.common.class_hash == remote_event.common.current_class_hash() {
            ResourceDiff::Synced(ResourceLocal::Event(self), remote)
        } else {
            ResourceDiff::Updated(ResourceLocal::Event(self), remote)
        }
    }
}

impl ComparableResource for NamespaceLocal {
    fn compare(self, remote: ResourceRemote) -> ResourceDiff {
        let remote_namespace = remote.as_namespace_or_panic();

        if self.name == remote_namespace.name {
            ResourceDiff::Synced(ResourceLocal::Namespace(self), remote)
        } else {
            unreachable!("Namespace should not be updated.")
        }
    }
}

impl ComparableResource for ResourceLocal {
    fn compare(self, remote: ResourceRemote) -> ResourceDiff {
        match self {
            ResourceLocal::Contract(contract) => contract.compare(remote),
            ResourceLocal::ExternalContract(contract) => contract.compare(remote),
            ResourceLocal::Model(model) => model.compare(remote),
            ResourceLocal::Event(event) => event.compare(remote),
            ResourceLocal::Namespace(ns) => ns.compare(remote),
            ResourceLocal::Library(library) => library.compare(remote),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use starknet::core::types::Felt;

    use super::*;
    use crate::local::{CommonLocalInfo, ContractLocal, EventLocal, ModelLocal};
    use crate::remote::{
        CommonRemoteInfo, ContractRemote, EventRemote, ModelRemote, NamespaceRemote,
    };
    use crate::test_utils::empty_sierra_class;

    #[test]
    fn test_compare_model_local() {
        let local_model = ModelLocal {
            common: CommonLocalInfo {
                name: "model1".to_string(),
                namespace: "ns1".to_string(),
                class: empty_sierra_class(),
                casm_class: None,
                class_hash: Felt::ZERO,
                casm_class_hash: Felt::ZERO,
            },
            members: vec![],
        };

        let mut remote_model = ResourceRemote::Model(ModelRemote {
            common: CommonRemoteInfo {
                class_hashes: vec![Felt::ZERO],
                name: "model1".to_string(),
                namespace: "ns1".to_string(),
                address: Felt::ZERO,
                owners: HashSet::new(),
                writers: HashSet::new(),
                metadata_hash: Felt::ZERO,
            },
        });

        let diff = local_model.clone().compare(remote_model.clone());
        assert!(matches!(diff, ResourceDiff::Synced(_, _)));

        // Upgrade the remote model.
        remote_model.push_class_hash(Felt::ONE);

        let diff_updated = local_model.compare(remote_model.clone());
        assert!(matches!(diff_updated, ResourceDiff::Updated(_, _)));
    }

    #[test]
    fn test_compare_event_local() {
        let local_event = EventLocal {
            common: CommonLocalInfo {
                name: "event1".to_string(),
                namespace: "ns1".to_string(),
                class: empty_sierra_class(),
                casm_class: None,
                class_hash: Felt::ZERO,
                casm_class_hash: Felt::ZERO,
            },
            members: vec![],
        };

        let mut remote_event = ResourceRemote::Event(EventRemote {
            common: CommonRemoteInfo {
                class_hashes: vec![Felt::ZERO],
                name: "event1".to_string(),
                namespace: "ns1".to_string(),
                address: Felt::ZERO,
                owners: HashSet::new(),
                writers: HashSet::new(),
                metadata_hash: Felt::ZERO,
            },
        });

        let diff = local_event.clone().compare(remote_event.clone());
        assert!(matches!(diff, ResourceDiff::Synced(_, _)));

        // Upgrade the remote event.
        remote_event.push_class_hash(Felt::ONE);

        let diff_updated = local_event.compare(remote_event.clone());
        assert!(matches!(diff_updated, ResourceDiff::Updated(_, _)));
    }

    #[test]
    fn test_compare_namespace_local() {
        let local_namespace = NamespaceLocal { name: "namespace1".to_string() };

        let remote_namespace = ResourceRemote::Namespace(NamespaceRemote {
            name: "namespace1".to_string(),
            owners: HashSet::new(),
            writers: HashSet::new(),
        });

        let diff = local_namespace.compare(remote_namespace.clone());
        assert!(matches!(diff, ResourceDiff::Synced(_, _)));
    }

    #[test]
    fn test_compare_contract_local() {
        let local_contract = ContractLocal {
            common: CommonLocalInfo {
                name: "contract1".to_string(),
                namespace: "ns1".to_string(),
                class: empty_sierra_class(),
                casm_class: None,
                class_hash: Felt::ZERO,
                casm_class_hash: Felt::ZERO,
            },
            systems: vec![],
        };

        let mut remote_contract = ResourceRemote::Contract(ContractRemote {
            common: CommonRemoteInfo {
                class_hashes: vec![Felt::ZERO],
                name: "contract1".to_string(),
                namespace: "ns1".to_string(),
                address: Felt::ZERO,
                owners: HashSet::new(),
                writers: HashSet::new(),
                metadata_hash: Felt::ZERO,
            },
            is_initialized: true,
        });

        let diff = local_contract.clone().compare(remote_contract.clone());
        assert!(matches!(diff, ResourceDiff::Synced(_, _)));

        remote_contract.push_class_hash(Felt::ONE);

        let diff_updated = local_contract.compare(remote_contract.clone());
        assert!(matches!(diff_updated, ResourceDiff::Updated(_, _)));
    }
}
