//! Represents the difference between a local and a remote world.
//!

use std::collections::{HashMap, HashSet};

use crate::{DojoSelector, Namespace};

use super::local::{ResourceLocal, WorldLocal};
use super::remote::{ResourceRemote, WorldRemote};

use compare::ComparableResource;

mod compare;

/// The difference between a local and a remote resource.
///
/// The point of view is the local one.
/// Currently, having the remote resources that are not registered by the current project is not supported,
/// since a world can be permissionlessly updated by anyone.
#[derive(Debug)]
pub enum DiffResource {
    /// The resource has been created locally, and is not present in the remote world.
    Created(ResourceLocal),
    /// The resource has been updated locally, and is different from the remote world.
    Updated(ResourceLocal, ResourceRemote),
    /// The local resource is in sync with the remote world.
    Synced(ResourceRemote),
}

pub struct WorldDiff {
    pub namespaces: Vec<DiffResource>,
    pub contracts: HashMap<Namespace, Vec<DiffResource>>,
    pub models: HashMap<Namespace, Vec<DiffResource>>,
    pub events: HashMap<Namespace, Vec<DiffResource>>,
}

impl WorldDiff {
    /// Creates a new world diff from a local and a remote world.
    ///
    /// Consumes the local and remote worlds to avoid duplicating the resources,
    /// since the [`DiffResource`] will contain one or both of the local and remote resources.
    pub fn new(mut local: WorldLocal, mut remote: WorldRemote) -> Self {
        let mut diff = Self {
            namespaces: vec![],
            contracts: HashMap::new(),
            models: HashMap::new(),
            events: HashMap::new(),
        };

        for local_ns in &local.namespaces {
            if remote.namespaces.contains(&local_ns) {
                diff.namespaces
                    .push(DiffResource::Synced(remote.resources.remove(&local_ns).unwrap()));
            } else {
                // The namespace is registered in the local world, safe to unwrap.
                diff.namespaces
                    .push(DiffResource::Created(local.resources.remove(&local_ns).unwrap()));
            }
        }

        compare_and_consume_resources(
            &local.contracts,
            &remote.contracts,
            &mut local.resources,
            &mut remote.resources,
            &mut diff.contracts,
        );

        compare_and_consume_resources(
            &local.models,
            &remote.models,
            &mut local.resources,
            &mut remote.resources,
            &mut diff.models,
        );

        compare_and_consume_resources(
            &local.events,
            &remote.events,
            &mut local.resources,
            &mut remote.resources,
            &mut diff.events,
        );

        diff
    }
}

/// Compares the local and remote resources and consumes them into a diff.
fn compare_and_consume_resources(
    local: &HashMap<Namespace, HashSet<DojoSelector>>,
    remote: &HashMap<Namespace, HashSet<DojoSelector>>,
    local_resources: &mut HashMap<DojoSelector, ResourceLocal>,
    remote_resources: &mut HashMap<DojoSelector, ResourceRemote>,
    diff: &mut HashMap<Namespace, Vec<DiffResource>>,
) {
    for (namespace, local_selectors) in local {
        for ls in local_selectors {
            let local_resource = local_resources.remove(ls).unwrap();

            let remote_selectors =
                if let Some(rss) = remote.get(namespace) { rss } else { &HashSet::new() };

            if remote_selectors.contains(ls) {
                diff.entry(namespace.clone()).or_default().push(
                    local_resource.compare(
                        remote_resources
                            .remove(ls)
                            .expect("Resource must exist if selector is present"),
                    ),
                );
            } else {
                diff.entry(namespace.clone())
                    .or_default()
                    .push(DiffResource::Created(local_resource));
            }
        }
    }
}
