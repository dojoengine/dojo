//! Implements the comparison between a local and a remote resource/world.
//!
//! The point of view is the local one.

use super::DiffResource;
use crate::local::{ContractLocal, EventLocal, ModelLocal, NamespaceLocal, ResourceLocal};
use crate::remote::ResourceRemote;

/// A trait to compare a local resource with a remote one.
pub trait ComparableResource {
    /// Compares a local resource with a remote one.
    ///
    /// Takes ownership since the [`DiffResource`] will contain one or both resources.
    fn compare(self, remote: ResourceRemote) -> DiffResource;
}

impl ComparableResource for ContractLocal {
    fn compare(self, remote: ResourceRemote) -> DiffResource {
        let remote_contract = remote.as_contract_or_panic();

        if self.class_hash == remote_contract.common.current_class_hash() {
            DiffResource::Synced(remote)
        } else {
            DiffResource::Updated(ResourceLocal::Contract(self), remote)
        }
    }
}

impl ComparableResource for ModelLocal {
    fn compare(self, remote: ResourceRemote) -> DiffResource {
        let remote_model = remote.as_model_or_panic();

        if self.class_hash == remote_model.common.current_class_hash() {
            DiffResource::Synced(remote)
        } else {
            DiffResource::Updated(ResourceLocal::Model(self), remote)
        }
    }
}

impl ComparableResource for EventLocal {
    fn compare(self, remote: ResourceRemote) -> DiffResource {
        let remote_event = remote.as_event_or_panic();

        if self.class_hash == remote_event.common.current_class_hash() {
            DiffResource::Synced(remote)
        } else {
            DiffResource::Updated(ResourceLocal::Event(self), remote)
        }
    }
}

impl ComparableResource for NamespaceLocal {
    fn compare(self, remote: ResourceRemote) -> DiffResource {
        let remote_namespace = remote.as_namespace_or_panic();

        if self.name == remote_namespace.name {
            DiffResource::Synced(remote)
        } else {
            unreachable!("Namespace should not be updated.")
        }
    }
}

impl ComparableResource for ResourceLocal {
    fn compare(self, remote: ResourceRemote) -> DiffResource {
        match self {
            ResourceLocal::Contract(contract) => contract.compare(remote),
            ResourceLocal::Model(model) => model.compare(remote),
            ResourceLocal::Event(event) => event.compare(remote),
            ResourceLocal::Namespace(ns) => ns.compare(remote),
            ResourceLocal::Starknet(_) => todo!("Starknet resources comparison."),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use starknet::core::types::Felt;

    use super::*;
    use crate::local::{ContractLocal, EventLocal, ModelLocal};
    use crate::remote::{
        CommonResourceRemoteInfo, ContractRemote, EventRemote, ModelRemote, NamespaceRemote,
    };
    use crate::test_utils::empty_sierra_class;

    #[test]
    fn test_compare_model_local() {
        let local_model = ModelLocal {
            name: "model1".to_string(),
            class: empty_sierra_class(),
            class_hash: Felt::ZERO,
        };

        let mut remote_model = ResourceRemote::Model(ModelRemote {
            common: CommonResourceRemoteInfo {
                class_hashes: vec![Felt::ZERO],
                name: "model1".to_string(),
                address: Felt::ZERO,
                owners: HashSet::new(),
                writers: HashSet::new(),
            },
        });

        let diff = local_model.clone().compare(remote_model.clone());
        assert!(matches!(diff, DiffResource::Synced(_)));

        // Upgrade the remote model.
        remote_model.push_class_hash(Felt::ONE);

        let diff_updated = local_model.compare(remote_model.clone());
        assert!(matches!(diff_updated, DiffResource::Updated(_, _)));
    }

    #[test]
    fn test_compare_event_local() {
        let local_event = EventLocal {
            name: "event1".to_string(),
            class: empty_sierra_class(),
            class_hash: Felt::ZERO,
        };

        let mut remote_event = ResourceRemote::Event(EventRemote {
            common: CommonResourceRemoteInfo {
                class_hashes: vec![Felt::ZERO],
                name: "event1".to_string(),
                address: Felt::ZERO,
                owners: HashSet::new(),
                writers: HashSet::new(),
            },
        });

        let diff = local_event.clone().compare(remote_event.clone());
        assert!(matches!(diff, DiffResource::Synced(_)));

        // Upgrade the remote event.
        remote_event.push_class_hash(Felt::ONE);

        let diff_updated = local_event.compare(remote_event.clone());
        assert!(matches!(diff_updated, DiffResource::Updated(_, _)));
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
        assert!(matches!(diff, DiffResource::Synced(_)));
    }

    #[test]
    fn test_compare_contract_local() {
        let local_contract = ContractLocal {
            name: "contract1".to_string(),
            class: empty_sierra_class(),
            class_hash: Felt::ZERO,
        };

        let mut remote_contract = ResourceRemote::Contract(ContractRemote {
            common: CommonResourceRemoteInfo {
                class_hashes: vec![Felt::ZERO],
                name: "contract1".to_string(),
                address: Felt::ZERO,
                owners: HashSet::new(),
                writers: HashSet::new(),
            },
            initialized: true,
        });

        let diff = local_contract.clone().compare(remote_contract.clone());
        assert!(matches!(diff, DiffResource::Synced(_)));

        remote_contract.push_class_hash(Felt::ONE);

        let diff_updated = local_contract.compare(remote_contract.clone());
        assert!(matches!(diff_updated, DiffResource::Updated(_, _)));
    }
}
