//! The migration module contains the logic for migrating the world.
//!
//! A migration is a sequence of steps that are executed in a specific order,
//! based on the [`WorldDiff`] that is computed from the local and remote world.
//!
//! Migrating a world can be sequenced as follows:
//!
//! 1. First the namespaces are synced.
//! 2. Then, all the resources (Contract, Models, Events) are synced, which can consist of:
//!    - Declaring the classes.
//!    - Registering the resources.
//!    - Upgrading the resources.
//! 3. Once resources are synced, the permissions are synced. Permissions can be in different states:
//!    - For newly registered resources, the permissions are applied.
//!    - For existing resources, the permissions are compared to the onchain state and the necessary changes are applied.
//! 4. All contracts that are not initialized are initialized, since permissions are applied, initialization of contracts can mutate resources.
//!

use std::collections::{HashMap, HashSet};

use cainome::cairo_serde::{ByteArray, ClassHash};
use declarer::Declarer;
use dojo_utils::{TransactionExt, TxnConfig};
use dojo_world::contracts::WorldContract;
use dojo_world::diff::{ResourceDiff, WorldDiff};
use dojo_world::local::ResourceLocal;
use dojo_world::remote::ResourceRemote;
use dojo_world::DojoSelector;
use starknet::accounts::{AccountError, ConnectedAccount};
use starknet::core::types::contract::ComputeClassHashError;
use starknet::core::types::Call;
use starknet::providers::ProviderError;
use starknet_crypto::Felt;
use thiserror::Error;

mod declarer;

#[derive(Debug)]
pub struct Migration<A>
where
    A: ConnectedAccount + Sync + Send,
{
    diff: WorldDiff,
    world: WorldContract<A>,
    txn_config: TxnConfig,
}

#[derive(Debug, Error)]
pub enum MigrationError<S> {
    #[error(transparent)]
    Migrator(#[from] AccountError<S>),
    #[error(transparent)]
    CairoSerde(#[from] cainome::cairo_serde::Error),
    #[error(transparent)]
    ComputeClassHash(#[from] ComputeClassHashError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    StarknetJson(#[from] starknet::core::types::contract::JsonError),
    /* #[error("Compiling contract.")]
    CompilingContract,
    #[error("Class already declared.")]
    ClassAlreadyDeclared,
    #[error("Contract already deployed.")]
    ContractAlreadyDeployed(Felt),
    #[error(transparent)]
    Migrator(#[from] AccountError<S>),
    #[error(transparent)]
    CairoShortStringToFelt(#[from] CairoShortStringToFeltError),
    #[error(transparent)]
    WaitingError(#[from] TransactionWaitingError),
    #[error(transparent)]o
    ArtifactError(#[from] anyhow::Error),
    #[error("Bad init calldata.")]
    BadInitCalldata, */
}

impl<A> Migration<A>
where
    A: ConnectedAccount + Sync + Send,
{
    pub fn new(diff: WorldDiff, world: WorldContract<A>, txn_config: TxnConfig) -> Self {
        Self { diff, world, txn_config }
    }

    pub async fn migrate(&self) -> Result<(), MigrationError<A::SignError>> {
        self.sync_resources().await?;

        Ok(())
    }

    /// Syncs the permissions.
    ///
    /// TODO: for error message, we need the name + namespace (or the tag for non-namespace resources).
    /// Change `DojoSelector` with a struct containing the local definition of an overlay resource,
    /// which can contain also writers.
    async fn sync_permissions(
        &self,
        local_writers: &HashMap<DojoSelector, HashSet<Felt>>,
        local_owners: &HashMap<DojoSelector, HashSet<Felt>>,
        force_local: bool,
    ) -> Result<(), MigrationError<A::SignError>> {
        let remote_writers = self.diff.get_remote_writers();
        let remote_owners = self.diff.get_remote_owners();

        for (dojo_selector, writers) in local_writers {
            let remote_writers = remote_writers.get(dojo_selector).unwrap_or(&HashSet::new());
            if force_local {
                self.world.set_permissions(dojo_selector, writers, HashSet::new()).await?;
            }
        }

        Ok(())
    }

    /// Syncs the resources by declaring the classes and registering/upgrading the resources.
    async fn sync_resources(&self) -> Result<(), MigrationError<A::SignError>> {
        let mut calls = vec![];
        let mut declarer = Declarer::new();

        self.namespaces_getcalls(&mut calls).await?;
        self.contracts_getcalls(&mut calls, &mut declarer).await?;
        self.models_getcalls(&mut calls, &mut declarer).await?;
        self.events_getcalls(&mut calls, &mut declarer).await?;

        // Sync resources.
        declarer.declare_all(&self.world.account, self.txn_config).await?;
        self.world.account.execute_v1(calls).send_with_cfg(&self.txn_config).await?;

        Ok(())
    }

    /// Returns the calls required to sync the namespaces.
    async fn namespaces_getcalls(
        &self,
        calls: &mut Vec<Call>,
    ) -> Result<(), MigrationError<A::SignError>> {
        for namespace in &self.diff.namespaces {
            if let ResourceDiff::Created(ResourceLocal::Namespace(namespace)) = namespace {
                calls.push(
                    self.world
                        .register_namespace_getcall(&ByteArray::from_string(&namespace.name)?),
                );
            }
        }

        Ok(())
    }

    /// Returns the calls required to sync the contracts and add the classes to the declarer.
    ///
    /// Currently, classes are cloned to be flattened, this is not ideal but the [`WorldDiff`]
    /// will be required later.
    /// If we could extract the info before syncing the resources, then we could avoid cloning the classes.
    async fn contracts_getcalls(
        &self,
        calls: &mut Vec<Call>,
        declarer: &mut Declarer,
    ) -> Result<(), MigrationError<A::SignError>> {
        for (namespace, contracts) in &self.diff.contracts {
            let ns_bytearray = ByteArray::from_string(&namespace)?;

            for contract in contracts {
                if let ResourceDiff::Created(ResourceLocal::Contract(contract)) = contract {
                    declarer.add_class(contract.casm_class_hash, contract.class.clone().flatten()?);

                    calls.push(self.world.register_contract_getcall(
                        &contract.salt(),
                        &ns_bytearray,
                        &ClassHash(contract.class_hash),
                    ));
                }

                if let ResourceDiff::Updated(
                    ResourceLocal::Contract(contract_local),
                    ResourceRemote::Contract(_contract_remote),
                ) = contract
                {
                    declarer.add_class(
                        contract_local.casm_class_hash,
                        contract_local.class.clone().flatten()?,
                    );

                    calls.push(self.world.upgrade_contract_getcall(
                        &ns_bytearray,
                        &ClassHash(contract_local.class_hash),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Returns the calls required to sync the models and add the classes to the declarer.
    async fn models_getcalls(
        &self,
        calls: &mut Vec<Call>,
        declarer: &mut Declarer,
    ) -> Result<(), MigrationError<A::SignError>> {
        for (namespace, models) in &self.diff.models {
            let ns_bytearray = ByteArray::from_string(&namespace)?;

            for model in models {
                if let ResourceDiff::Created(ResourceLocal::Model(model)) = model {
                    declarer.add_class(model.casm_class_hash, model.class.clone().flatten()?);

                    calls.push(
                        self.world
                            .register_model_getcall(&ns_bytearray, &ClassHash(model.class_hash)),
                    );
                }

                if let ResourceDiff::Updated(
                    ResourceLocal::Model(model_local),
                    ResourceRemote::Model(_model_remote),
                ) = model
                {
                    declarer.add_class(
                        model_local.casm_class_hash,
                        model_local.class.clone().flatten()?,
                    );

                    calls.push(
                        self.world.upgrade_model_getcall(
                            &ns_bytearray,
                            &ClassHash(model_local.class_hash),
                        ),
                    );
                }
            }
        }

        Ok(())
    }

    /// Returns the calls required to sync the events and add the classes to the declarer.
    async fn events_getcalls(
        &self,
        calls: &mut Vec<Call>,
        declarer: &mut Declarer,
    ) -> Result<(), MigrationError<A::SignError>> {
        for (namespace, events) in &self.diff.events {
            let ns_bytearray = ByteArray::from_string(&namespace)?;

            for event in events {
                if let ResourceDiff::Created(ResourceLocal::Event(event)) = event {
                    declarer.add_class(event.casm_class_hash, event.class.clone().flatten()?);

                    calls.push(
                        self.world
                            .register_event_getcall(&ns_bytearray, &ClassHash(event.class_hash)),
                    );
                }

                if let ResourceDiff::Updated(
                    ResourceLocal::Event(event_local),
                    ResourceRemote::Event(_event_remote),
                ) = event
                {
                    declarer.add_class(
                        event_local.casm_class_hash,
                        event_local.class.clone().flatten()?,
                    );

                    calls.push(
                        self.world.upgrade_event_getcall(
                            &ns_bytearray,
                            &ClassHash(event_local.class_hash),
                        ),
                    );
                }
            }
        }

        Ok(())
    }
}
