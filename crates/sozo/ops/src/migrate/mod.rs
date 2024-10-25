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
//! 3. Once resources are synced, the permissions are synced. Permissions can be in different
//!    states:
//!    - For newly registered resources, the permissions are applied.
//!    - For existing resources, the permissions are compared to the onchain state and the necessary
//!      changes are applied.
//! 4. All contracts that are not initialized are initialized, since permissions are applied,
//!    initialization of contracts can mutate resources.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use cainome::cairo_serde::{ByteArray, ClassHash, ContractAddress};
use declarer::Declarer;
use deployer::Deployer;
use dojo_types::naming;
use dojo_utils::TxnConfig;
use dojo_world::config::ProfileConfig;
use dojo_world::contracts::WorldContract;
use dojo_world::diff::{ResourceDiff, WorldDiff, WorldStatus};
use dojo_world::local::ResourceLocal;
use dojo_world::remote::ResourceRemote;
use dojo_world::utils;
use invoker::Invoker;
use starknet::accounts::ConnectedAccount;
use starknet_crypto::Felt;
use tracing::trace;

// TODO: those may be moved to dojo-utils in the tx module.
pub mod declarer;
pub mod deployer;
pub mod error;
pub mod invoker;

pub use error::MigrationError;

#[derive(Debug)]
pub struct Migration<A>
where
    A: ConnectedAccount + Sync + Send,
{
    diff: WorldDiff,
    world: WorldContract<A>,
    txn_config: TxnConfig,
    profile_config: ProfileConfig,
}

impl<A> Migration<A>
where
    A: ConnectedAccount + Sync + Send,
{
    /// Creates a new migration.
    pub fn new(
        diff: WorldDiff,
        world: WorldContract<A>,
        txn_config: TxnConfig,
        profile_config: ProfileConfig,
    ) -> Self {
        Self { diff, world, txn_config, profile_config }
    }

    /// Migrates the world by syncing the namespaces, resources, permissions and initializing the
    /// contracts.
    pub async fn migrate(&self) -> Result<(), MigrationError<A::SignError>> {
        self.ensure_world().await?;
        self.sync_resources().await?;
        self.sync_permissions().await?;
        self.initialize_contracts().await?;

        Ok(())
    }

    /// Returns whether multicall should be used. By default, it is enabled.
    fn do_multicall(&self) -> bool {
        self.profile_config.migration.as_ref().map_or(true, |m| !m.disable_multicall)
    }

    /// For all contracts that are not initialized, initialize them by using the init call arguments
    /// found in the [`ProfileConfig`].
    async fn initialize_contracts(&self) -> Result<(), MigrationError<A::SignError>> {
        let mut invoker = Invoker::new(&self.world.account, self.txn_config.clone());

        let init_call_args = if let Some(init_call_args) = &self.profile_config.init_call_args {
            init_call_args.clone()
        } else {
            HashMap::new()
        };

        for (namespace, contracts) in &self.diff.contracts {
            for contract in contracts {
                let (do_init, selector, init_call_args, tag) = match contract {
                    ResourceDiff::Created(ResourceLocal::Contract(contract)) => {
                        let tag = naming::get_tag(namespace, &contract.name);
                        (
                            true,
                            contract.dojo_selector(namespace),
                            init_call_args.get(&tag).clone(),
                            tag,
                        )
                    }
                    ResourceDiff::Updated(_, ResourceRemote::Contract(contract)) => {
                        let tag = naming::get_tag(namespace, &contract.common.name);
                        (
                            !contract.is_initialized,
                            contract.dojo_selector(namespace),
                            init_call_args.get(&tag).clone(),
                            tag,
                        )
                    }
                    ResourceDiff::Synced(ResourceRemote::Contract(contract)) => {
                        let tag = naming::get_tag(namespace, &contract.common.name);
                        (
                            !contract.is_initialized,
                            contract.dojo_selector(namespace),
                            init_call_args.get(&tag).clone(),
                            tag,
                        )
                    }
                    _ => (false, Felt::ZERO, None, String::new()),
                };

                if do_init {
                    // Currently, only felts are supported in the init call data.
                    // The injection of class hash and addresses is no longer supported since the
                    // world contains an internal DNS.
                    let args = if let Some(args) = init_call_args {
                        let mut parsed_args = vec![];
                        for arg in args {
                            parsed_args.push(Felt::from_str(arg)?);
                        }
                        parsed_args
                    } else {
                        vec![]
                    };

                    trace!(tag, ?args, "Initializing contract.");

                    invoker.add_call(self.world.init_contract_getcall(&selector, &args));
                }
            }
        }

        if self.do_multicall() {
            invoker.multicall().await?;
        } else {
            invoker.invoke_all_sequentially().await?;
        }

        Ok(())
    }

    /// Syncs the permissions.
    ///
    /// This first version is naive, and only applies the local permissions to the resources, if the
    /// permission is not already set onchain.
    ///
    /// TODO: An other function must be added to sync the remote permissions to the local ones,
    /// and allow the user to reset the permissions onchain to the local ones.
    ///
    /// TODO: for error message, we need the name + namespace (or the tag for non-namespace
    /// resources). Change `DojoSelector` with a struct containing the local definition of an
    /// overlay resource, which can contain also writers.
    async fn sync_permissions(&self) -> Result<(), MigrationError<A::SignError>> {
        // The remote writers and owners are already containing addresses.
        let remote_writers = self.diff.get_remote_writers();
        let remote_owners = self.diff.get_remote_owners();

        // The local writers and owners are containing only selectors and not the addresses.
        // A mapping is required to then give the permissions to the right addresses.
        let local_writers = self.profile_config.get_local_writers();
        let local_owners = self.profile_config.get_local_owners();

        // For all contracts in a dojo project, addresses are deterministic.
        let contract_addresses = self.diff.get_contracts_addresses(self.world.address);

        let mut invoker = Invoker::new(&self.world.account, self.txn_config.clone());

        // For all local writer/owner that is not found remotely, we need to grant the permission.
        for (target_selector, granted_selectors) in local_writers {
            for (granted_selector, tag) in granted_selectors {
                let granted_address = contract_addresses
                    .get(&granted_selector)
                    .ok_or(MigrationError::OrphanSelectorAddress(tag))?;

                if !remote_writers
                    .get(&target_selector)
                    .as_ref()
                    .unwrap_or(&&HashSet::new())
                    .contains(granted_address)
                {
                    invoker.add_call(self.world.grant_writer_getcall(
                        &target_selector,
                        &ContractAddress(*granted_address),
                    ));
                }
            }
        }

        for (target_selector, granted_selectors) in local_owners {
            for (granted_selector, tag) in granted_selectors {
                let granted_address = contract_addresses
                    .get(&granted_selector)
                    .ok_or(MigrationError::OrphanSelectorAddress(tag))?;

                if !remote_owners
                    .get(&target_selector)
                    .as_ref()
                    .unwrap_or(&&HashSet::new())
                    .contains(granted_address)
                {
                    invoker.add_call(
                        self.world.grant_owner_getcall(
                            &target_selector,
                            &ContractAddress(*granted_address),
                        ),
                    );
                }
            }
        }

        if self.do_multicall() {
            invoker.multicall().await?;
        } else {
            invoker.invoke_all_sequentially().await?;
        }

        Ok(())
    }

    /// Syncs the resources by declaring the classes and registering/upgrading the resources.
    async fn sync_resources(&self) -> Result<(), MigrationError<A::SignError>> {
        let mut invoker = Invoker::new(&self.world.account, self.txn_config.clone());
        let mut declarer = Declarer::new(&self.world.account, self.txn_config.clone());

        self.namespaces_getcalls(&mut invoker).await?;
        self.contracts_getcalls(&mut invoker, &mut declarer).await?;
        self.models_getcalls(&mut invoker, &mut declarer).await?;
        self.events_getcalls(&mut invoker, &mut declarer).await?;

        declarer.declare_all().await?;

        if self.do_multicall() {
            invoker.multicall().await?;
        } else {
            invoker.invoke_all_sequentially().await?;
        }

        Ok(())
    }

    /// Returns the calls required to sync the namespaces.
    async fn namespaces_getcalls(
        &self,
        invoker: &mut Invoker<&A>,
    ) -> Result<(), MigrationError<A::SignError>> {
        for namespace in &self.diff.namespaces {
            if let ResourceDiff::Created(ResourceLocal::Namespace(namespace)) = namespace {
                trace!(name = namespace.name, "Registering namespace.");

                invoker.add_call(
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
    /// If we could extract the info before syncing the resources, then we could avoid cloning the
    /// classes.
    async fn contracts_getcalls(
        &self,
        invoker: &mut Invoker<&A>,
        declarer: &mut Declarer<&A>,
    ) -> Result<(), MigrationError<A::SignError>> {
        for (namespace, contracts) in &self.diff.contracts {
            let ns_bytearray = ByteArray::from_string(&namespace)?;

            for contract in contracts {
                if let ResourceDiff::Created(ResourceLocal::Contract(contract)) = contract {
                    trace!(
                        namespace,
                        name = contract.name,
                        class_hash = format!("{:#066x}", contract.class_hash),
                        "Registering contract."
                    );

                    declarer.add_class(contract.casm_class_hash, contract.class.clone().flatten()?);

                    invoker.add_call(self.world.register_contract_getcall(
                        &contract.dojo_selector(&namespace),
                        &ns_bytearray,
                        &ClassHash(contract.class_hash),
                    ));
                }

                if let ResourceDiff::Updated(
                    ResourceLocal::Contract(contract_local),
                    ResourceRemote::Contract(_contract_remote),
                ) = contract
                {
                    trace!(
                        namespace,
                        name = contract_local.name,
                        class_hash = format!("{:#066x}", contract_local.class_hash),
                        "Upgrading contract."
                    );

                    declarer.add_class(
                        contract_local.casm_class_hash,
                        contract_local.class.clone().flatten()?,
                    );

                    invoker.add_call(self.world.upgrade_contract_getcall(
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
        invoker: &mut Invoker<&A>,
        declarer: &mut Declarer<&A>,
    ) -> Result<(), MigrationError<A::SignError>> {
        for (namespace, models) in &self.diff.models {
            let ns_bytearray = ByteArray::from_string(&namespace)?;

            for model in models {
                if let ResourceDiff::Created(ResourceLocal::Model(model)) = model {
                    trace!(
                        namespace,
                        name = model.name,
                        class_hash = format!("{:#066x}", model.class_hash),
                        "Registering model."
                    );

                    declarer.add_class(model.casm_class_hash, model.class.clone().flatten()?);

                    invoker.add_call(
                        self.world
                            .register_model_getcall(&ns_bytearray, &ClassHash(model.class_hash)),
                    );
                }

                if let ResourceDiff::Updated(
                    ResourceLocal::Model(model_local),
                    ResourceRemote::Model(_model_remote),
                ) = model
                {
                    trace!(
                        namespace,
                        name = model_local.name,
                        class_hash = format!("{:#066x}", model_local.class_hash),
                        "Upgrading model."
                    );

                    declarer.add_class(
                        model_local.casm_class_hash,
                        model_local.class.clone().flatten()?,
                    );

                    invoker.add_call(
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
        invoker: &mut Invoker<&A>,
        declarer: &mut Declarer<&A>,
    ) -> Result<(), MigrationError<A::SignError>> {
        for (namespace, events) in &self.diff.events {
            let ns_bytearray = ByteArray::from_string(&namespace)?;

            for event in events {
                if let ResourceDiff::Created(ResourceLocal::Event(event)) = event {
                    trace!(
                        namespace,
                        name = event.name,
                        class_hash = format!("{:#066x}", event.class_hash),
                        "Registering event."
                    );

                    declarer.add_class(event.casm_class_hash, event.class.clone().flatten()?);

                    invoker.add_call(
                        self.world
                            .register_event_getcall(&ns_bytearray, &ClassHash(event.class_hash)),
                    );
                }

                if let ResourceDiff::Updated(
                    ResourceLocal::Event(event_local),
                    ResourceRemote::Event(_event_remote),
                ) = event
                {
                    trace!(
                        namespace,
                        name = event_local.name,
                        class_hash = format!("{:#066x}", event_local.class_hash),
                        "Upgrading event."
                    );

                    declarer.add_class(
                        event_local.casm_class_hash,
                        event_local.class.clone().flatten()?,
                    );

                    invoker.add_call(
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

    /// Ensures the world is declared and deployed if necessary.
    async fn ensure_world(&self) -> Result<(), MigrationError<A::SignError>> {
        if let WorldStatus::NewVersion(class_hash, casm_class_hash, class) = &self.diff.world_status
        {
            trace!("Declaring and deploying world.");

            Declarer::declare(
                *casm_class_hash,
                class.clone().flatten()?,
                &self.world.account,
                &self.txn_config,
            )
            .await?;

            let deployer = Deployer::new(&self.world.account, self.txn_config.clone());

            deployer
                .deploy_via_udc(
                    *class_hash,
                    utils::world_salt(&self.profile_config.world.seed)?,
                    &[*class_hash],
                    Felt::ZERO,
                )
                .await?;
        }

        Ok(())
    }
}
