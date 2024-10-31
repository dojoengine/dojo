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

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use cainome::cairo_serde::{ByteArray, ClassHash, ContractAddress};
use dojo_utils::{Declarer, Deployer, Invoker, TxnConfig};
use dojo_world::config::ProfileConfig;
use dojo_world::contracts::WorldContract;
use dojo_world::diff::{Manifest, ResourceDiff, WorldDiff, WorldStatus};
use dojo_world::local::ResourceLocal;
use dojo_world::remote::ResourceRemote;
use dojo_world::{utils, ResourceType};
use spinoff::Spinner;
use starknet::accounts::{ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::{Call, FlattenedSierraClass};
use starknet::providers::AnyProvider;
use starknet::signers::LocalWallet;
use starknet_crypto::Felt;
use tracing::trace;

pub mod error;
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
    // This is only to retrieve the declarers or make custom calls.
    // Ideally, we want this rpc url to be exposed from the world.account.provider().
    rpc_url: String,
}

#[derive(Debug)]
pub struct MigrationResult {
    pub has_changes: bool,
    pub manifest: Manifest,
}

pub enum MigrationUi {
    Spinner(Spinner),
    None,
}

impl fmt::Debug for MigrationUi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spinner(_) => write!(f, "Spinner"),
            Self::None => write!(f, "None"),
        }
    }
}

impl MigrationUi {
    pub fn update_text(&mut self, text: &'static str) {
        match self {
            Self::Spinner(s) => s.update_text(text),
            Self::None => (),
        }
    }

    pub fn stop_and_persist(&mut self, symbol: &'static str, text: &'static str) {
        match self {
            Self::Spinner(s) => s.stop_and_persist(symbol, text),
            Self::None => (),
        }
    }
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
        rpc_url: String,
    ) -> Self {
        Self { diff, world, txn_config, profile_config, rpc_url }
    }

    /// Migrates the world by syncing the namespaces, resources, permissions and initializing the
    /// contracts.
    ///
    /// TODO: find a more elegant way to pass an UI printer to the ops library than a hard coded
    /// spinner.
    pub async fn migrate(
        &self,
        spinner: &mut MigrationUi,
    ) -> Result<MigrationResult, MigrationError<A::SignError>> {
        spinner.update_text("Deploying world...");
        let world_has_changed = self.ensure_world().await?;

        let resources_have_changed = if !self.diff.is_synced() {
            spinner.update_text("Syncing resources...");
            self.sync_resources().await?
        } else {
            false
        };

        spinner.update_text("Syncing permissions...");
        let permissions_have_changed = self.sync_permissions().await?;

        spinner.update_text("Initializing contracts...");
        let contracts_have_changed = self.initialize_contracts().await?;

        Ok(MigrationResult {
            has_changes: world_has_changed
                || resources_have_changed
                || permissions_have_changed
                || contracts_have_changed,
            manifest: Manifest::new(&self.diff),
        })
    }

    /// Returns whether multicall should be used. By default, it is enabled.
    fn do_multicall(&self) -> bool {
        self.profile_config
            .migration
            .as_ref()
            .map_or(true, |m| !m.disable_multicall.unwrap_or(false))
    }

    /// For all contracts that are not initialized, initialize them by using the init call arguments
    /// found in the [`ProfileConfig`].
    ///
    /// Returns true if at least one contract has been initialized, false otherwise.
    async fn initialize_contracts(&self) -> Result<bool, MigrationError<A::SignError>> {
        let mut invoker = Invoker::new(&self.world.account, self.txn_config);

        let init_call_args = if let Some(init_call_args) = &self.profile_config.init_call_args {
            init_call_args.clone()
        } else {
            HashMap::new()
        };

        // Ensure we can order the contracts to initialize, if specified.
        // Keeps the tag matched to the call to initialize.
        let ordered_init_tags = self
            .profile_config
            .migration
            .as_ref()
            .map_or(vec![], |m| m.order_inits.clone().unwrap_or_default());

        // Keeps map between the order index and the call to initialize.
        let mut ordered_init_calls = HashMap::new();

        for (selector, resource) in &self.diff.resources {
            if resource.resource_type() == ResourceType::Contract {
                let tag = resource.tag();

                // TODO: maybe we want a resource diff with a new variant. Where the migration
                // is skipped, but we still have the local resource.
                if self.profile_config.is_skipped(&tag) {
                    continue;
                }

                let (do_init, init_call_args) = match resource {
                    ResourceDiff::Created(ResourceLocal::Contract(_)) => {
                        (true, init_call_args.get(&tag))
                    }
                    ResourceDiff::Updated(_, ResourceRemote::Contract(contract)) => {
                        (!contract.is_initialized, init_call_args.get(&tag))
                    }
                    ResourceDiff::Synced(_, ResourceRemote::Contract(contract)) => {
                        (!contract.is_initialized, init_call_args.get(&tag))
                    }
                    _ => (false, None),
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

                    if let Some(order_index) = ordered_init_tags.iter().position(|t| *t == tag) {
                        ordered_init_calls
                            .insert(order_index, self.world.init_contract_getcall(selector, &args));
                    } else {
                        invoker.add_call(self.world.init_contract_getcall(selector, &args));
                    }
                }
            }
        }

        if !ordered_init_calls.is_empty() {
            let mut ordered_keys: Vec<_> = ordered_init_calls.keys().cloned().collect();
            ordered_keys.sort();

            let ordered_calls: Vec<_> = ordered_keys
                .into_iter()
                .map(|k| ordered_init_calls.get(&k).expect("Ordered call must exist.").clone())
                .collect();

            invoker.extends_ordered(ordered_calls);
        }

        let has_changed = !invoker.calls.is_empty();

        if self.do_multicall() {
            invoker.multicall().await?;
        } else {
            invoker.invoke_all_sequentially().await?;
        }

        Ok(has_changed)
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
    ///
    /// Returns true if at least one permission has changed, false otherwise.
    async fn sync_permissions(&self) -> Result<bool, MigrationError<A::SignError>> {
        let mut invoker = Invoker::new(&self.world.account, self.txn_config);

        // Only takes the local permissions that are not already set onchain to apply them.
        for (selector, resource) in &self.diff.resources {
            if self.profile_config.is_skipped(&resource.tag()) {
                continue;
            }

            for pdiff in self.diff.get_writers(*selector).only_local() {
                trace!(
                    target = resource.tag(),
                    grantee_tag = pdiff.tag.unwrap_or_default(),
                    grantee_address = format!("{:#066x}", pdiff.address),
                    "Granting writer permission."
                );

                invoker.add_call(
                    self.world.grant_writer_getcall(selector, &ContractAddress(pdiff.address)),
                );
            }

            for pdiff in self.diff.get_owners(*selector).only_local() {
                trace!(
                    target = resource.tag(),
                    grantee_tag = pdiff.tag.unwrap_or_default(),
                    grantee_address = format!("{:#066x}", pdiff.address),
                    "Granting owner permission."
                );

                invoker.add_call(
                    self.world.grant_owner_getcall(selector, &ContractAddress(pdiff.address)),
                );
            }
        }

        let has_changed = !invoker.calls.is_empty();

        if self.do_multicall() {
            invoker.multicall().await?;
        } else {
            invoker.invoke_all_sequentially().await?;
        }

        Ok(has_changed)
    }

    /// Syncs the resources by declaring the classes and registering/upgrading the resources.
    ///
    /// Returns true if at least one resource has changed, false otherwise.
    async fn sync_resources(&self) -> Result<bool, MigrationError<A::SignError>> {
        let mut invoker = Invoker::new(&self.world.account, self.txn_config);

        // Namespaces must be synced first, since contracts, models and events are namespaced.
        self.namespaces_getcalls(&mut invoker).await?;

        let mut classes: HashMap<Felt, FlattenedSierraClass> = HashMap::new();

        // Collects the calls and classes to be declared to sync the resources.
        for resource in self.diff.resources.values() {
            if self.profile_config.is_skipped(&resource.tag()) {
                continue;
            }

            match resource.resource_type() {
                ResourceType::Contract => {
                    let (contract_calls, contract_classes) =
                        self.contracts_calls_classes(resource).await?;
                    invoker.extend_calls(contract_calls);
                    classes.extend(contract_classes);
                }
                ResourceType::Model => {
                    let (model_calls, model_classes) = self.models_calls_classes(resource).await?;
                    invoker.extend_calls(model_calls);
                    classes.extend(model_classes);
                }
                ResourceType::Event => {
                    let (event_calls, event_classes) = self.events_calls_classes(resource).await?;
                    invoker.extend_calls(event_calls);
                    classes.extend(event_classes);
                }
                _ => continue,
            }
        }

        let has_classes = !classes.is_empty();
        let has_calls = !invoker.calls.is_empty();
        let has_changed = has_classes || has_calls;

        // Declaration can be slow, and can be speed up by using multiple accounts.
        // Since migrator account from `self.world.account` is under the [`ConnectedAccount`] trait,
        // we can group it with the predeployed accounts which are concrete types.
        let accounts = self.get_accounts().await;

        if accounts.is_empty() {
            trace!("Declaring classes with migrator account.");
            let mut declarer = Declarer::new(&self.world.account, self.txn_config);
            declarer.extend_classes(classes.into_iter().collect());
            declarer.declare_all().await?;
        } else {
            trace!("Declaring classes with {} accounts.", accounts.len());
            let mut declarers = vec![];
            for account in accounts {
                declarers.push(Declarer::new(account, self.txn_config));
            }

            for (idx, (casm_class_hash, class)) in classes.into_iter().enumerate() {
                let declarer_idx = idx % declarers.len();
                declarers[declarer_idx].add_class(casm_class_hash, class);
            }

            let declarers_futures =
                futures::future::join_all(declarers.into_iter().map(|d| d.declare_all())).await;

            for declarer_results in declarers_futures {
                if let Err(e) = declarer_results {
                    // The issue is that `e` is bound to concrete type `SingleOwnerAccount`.
                    // Thus, we can't return `e` directly.
                    // Might have a better solution by addind a new variant?
                    if e.to_string().contains("Class already declared") {
                        // If the class is already declared, it might be because it was already
                        // declared in a previous run or an other declarer.
                        continue;
                    }

                    return Err(MigrationError::DeclareClassError(e.to_string()));
                }
            }
        }

        if self.do_multicall() {
            invoker.multicall().await?;
        } else {
            invoker.invoke_all_sequentially().await?;
        }

        Ok(has_changed)
    }

    /// Returns the calls required to sync the namespaces.
    async fn namespaces_getcalls(
        &self,
        invoker: &mut Invoker<&A>,
    ) -> Result<(), MigrationError<A::SignError>> {
        for namespace_selector in &self.diff.namespaces {
            // TODO: abstract this expect by having a function exposed in the diff.
            let resource =
                self.diff.resources.get(namespace_selector).expect("Namespace not found in diff.");

            if let ResourceDiff::Created(ResourceLocal::Namespace(namespace)) = resource {
                trace!(name = namespace.name, "Registering namespace.");

                invoker.add_call(
                    self.world
                        .register_namespace_getcall(&ByteArray::from_string(&namespace.name)?),
                );
            }
        }

        Ok(())
    }

    /// Gathers the calls required to sync the contracts and classes to be declared.
    ///
    /// Currently, classes are cloned to be flattened, this is not ideal but the [`WorldDiff`]
    /// will be required later.
    /// If we could extract the info before syncing the resources, then we could avoid cloning the
    /// classes.
    ///
    /// Returns a tuple of calls and (casm_class_hash, class) to be declared.
    async fn contracts_calls_classes(
        &self,
        resource: &ResourceDiff,
    ) -> Result<(Vec<Call>, HashMap<Felt, FlattenedSierraClass>), MigrationError<A::SignError>>
    {
        let mut calls = vec![];
        let mut classes = HashMap::new();

        let namespace = resource.namespace();
        let ns_bytearray = ByteArray::from_string(&namespace)?;

        if let ResourceDiff::Created(ResourceLocal::Contract(contract)) = resource {
            trace!(
                namespace,
                name = contract.common.name,
                class_hash = format!("{:#066x}", contract.common.class_hash),
                "Registering contract."
            );

            classes
                .insert(contract.common.casm_class_hash, contract.common.class.clone().flatten()?);

            calls.push(self.world.register_contract_getcall(
                &contract.dojo_selector(),
                &ns_bytearray,
                &ClassHash(contract.common.class_hash),
            ));
        }

        if let ResourceDiff::Updated(
            ResourceLocal::Contract(contract_local),
            ResourceRemote::Contract(_contract_remote),
        ) = resource
        {
            trace!(
                namespace,
                name = contract_local.common.name,
                class_hash = format!("{:#066x}", contract_local.common.class_hash),
                "Upgrading contract."
            );

            classes.insert(
                contract_local.common.casm_class_hash,
                contract_local.common.class.clone().flatten()?,
            );

            calls.push(self.world.upgrade_contract_getcall(
                &ns_bytearray,
                &ClassHash(contract_local.common.class_hash),
            ));
        }

        Ok((calls, classes))
    }

    /// Returns the calls required to sync the models and gather the classes to be declared.
    ///
    /// Returns a tuple of calls and (casm_class_hash, class) to be declared.
    async fn models_calls_classes(
        &self,
        resource: &ResourceDiff,
    ) -> Result<(Vec<Call>, HashMap<Felt, FlattenedSierraClass>), MigrationError<A::SignError>>
    {
        let mut calls = vec![];
        let mut classes = HashMap::new();

        let namespace = resource.namespace();
        let ns_bytearray = ByteArray::from_string(&namespace)?;

        if let ResourceDiff::Created(ResourceLocal::Model(model)) = resource {
            trace!(
                namespace,
                name = model.common.name,
                class_hash = format!("{:#066x}", model.common.class_hash),
                "Registering model."
            );

            classes.insert(model.common.casm_class_hash, model.common.class.clone().flatten()?);

            calls.push(
                self.world
                    .register_model_getcall(&ns_bytearray, &ClassHash(model.common.class_hash)),
            );
        }

        if let ResourceDiff::Updated(
            ResourceLocal::Model(model_local),
            ResourceRemote::Model(_model_remote),
        ) = resource
        {
            trace!(
                namespace,
                name = model_local.common.name,
                class_hash = format!("{:#066x}", model_local.common.class_hash),
                "Upgrading model."
            );

            classes.insert(
                model_local.common.casm_class_hash,
                model_local.common.class.clone().flatten()?,
            );

            calls.push(
                self.world.upgrade_model_getcall(
                    &ns_bytearray,
                    &ClassHash(model_local.common.class_hash),
                ),
            );
        }

        Ok((calls, classes))
    }

    /// Returns the calls required to sync the events and gather the classes to be declared.
    ///
    /// Returns a tuple of calls and (casm_class_hash, class) to be declared.
    async fn events_calls_classes(
        &self,
        resource: &ResourceDiff,
    ) -> Result<(Vec<Call>, HashMap<Felt, FlattenedSierraClass>), MigrationError<A::SignError>>
    {
        let mut calls = vec![];
        let mut classes = HashMap::new();

        let namespace = resource.namespace();
        let ns_bytearray = ByteArray::from_string(&namespace)?;

        if let ResourceDiff::Created(ResourceLocal::Event(event)) = resource {
            trace!(
                namespace,
                name = event.common.name,
                class_hash = format!("{:#066x}", event.common.class_hash),
                "Registering event."
            );

            classes.insert(event.common.casm_class_hash, event.common.class.clone().flatten()?);

            calls.push(
                self.world
                    .register_event_getcall(&ns_bytearray, &ClassHash(event.common.class_hash)),
            );
        }

        if let ResourceDiff::Updated(
            ResourceLocal::Event(event_local),
            ResourceRemote::Event(_event_remote),
        ) = resource
        {
            trace!(
                namespace,
                name = event_local.common.name,
                class_hash = format!("{:#066x}", event_local.common.class_hash),
                "Upgrading event."
            );

            classes.insert(
                event_local.common.casm_class_hash,
                event_local.common.class.clone().flatten()?,
            );

            calls.push(
                self.world.upgrade_event_getcall(
                    &ns_bytearray,
                    &ClassHash(event_local.common.class_hash),
                ),
            );
        }

        Ok((calls, classes))
    }

    /// Ensures the world is declared and deployed if necessary.
    ///
    /// Returns true if the world has to be deployed/updated, false otherwise.
    async fn ensure_world(&self) -> Result<bool, MigrationError<A::SignError>> {
        match &self.diff.world_info.status {
            WorldStatus::Synced => return Ok(false),
            WorldStatus::NotDeployed => {
                trace!("Deploying the first world.");

                Declarer::declare(
                    self.diff.world_info.casm_class_hash,
                    self.diff.world_info.class.clone().flatten()?,
                    &self.world.account,
                    &self.txn_config,
                )
                .await?;

                let deployer = Deployer::new(&self.world.account, self.txn_config);

                deployer
                    .deploy_via_udc(
                        self.diff.world_info.class_hash,
                        utils::world_salt(&self.profile_config.world.seed)?,
                        &[self.diff.world_info.class_hash],
                        Felt::ZERO,
                    )
                    .await?;
            }
            WorldStatus::NewVersion => {
                trace!("Upgrading the world.");

                Declarer::declare(
                    self.diff.world_info.casm_class_hash,
                    self.diff.world_info.class.clone().flatten()?,
                    &self.world.account,
                    &self.txn_config,
                )
                .await?;

                let mut invoker = Invoker::new(&self.world.account, self.txn_config);

                invoker.add_call(
                    self.world.upgrade_getcall(&ClassHash(self.diff.world_info.class_hash)),
                );

                invoker.multicall().await?;
            }
        };

        Ok(true)
    }

    /// Returns the accounts to use for the migration.
    ///
    /// This is useful to use multiple accounts since the declare transaction is nonce-based,
    /// and can only be parallelized by using different accounts.
    ///
    /// Accounts can come from the profile config, otherwise we fallback to the predeployed
    /// accounts.
    async fn get_accounts(&self) -> Vec<SingleOwnerAccount<AnyProvider, LocalWallet>> {
        // TODO: if profile config have some migrators, use them instead.

        // If the RPC provider does not support the predeployed accounts, this will fail silently.
        dojo_utils::get_predeployed_accounts(&self.world.account, &self.rpc_url)
            .await
            .unwrap_or_default()
    }
}
