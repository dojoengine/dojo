use std::collections::HashMap;
use std::{fs, io};

use anyhow::Result;
use cainome::cairo_serde::Error as CainomeError;
use camino::Utf8PathBuf;
use serde::de::DeserializeOwned;
use smol_str::SmolStr;
use starknet::core::types::{
    BlockId, BlockTag, EmittedEvent, EventFilter, Felt, FunctionCall, StarknetError,
};
use starknet::core::utils::{
    parse_cairo_short_string, starknet_keccak, CairoShortStringToFeltError,
    ParseCairoShortStringError,
};
use starknet::macros::selector;
use starknet::providers::{Provider, ProviderError};
use thiserror::Error;
use toml;
use tracing::error;

use crate::contracts::model::ModelError;
use crate::contracts::world::WorldEvent;
use crate::contracts::WorldContractReader;

#[cfg(test)]
#[path = "manifest_test.rs"]
mod test;

mod types;

pub use types::{
    AbiFormat, BaseManifest, Class, ComputedValueEntrypoint, DeploymentManifest, DojoContract,
    DojoModel, Manifest, ManifestMethods, Member, OverlayClass, OverlayContract,
    OverlayDojoContract, OverlayDojoModel, OverlayManifest, WorldContract, WorldMetadata,
};

pub const WORLD_CONTRACT_NAME: &str = "dojo::world::world";
pub const BASE_CONTRACT_NAME: &str = "dojo::base::base";
pub const RESOURCE_METADATA_CONTRACT_NAME: &str = "dojo::resource_metadata::resource_metadata";
pub const RESOURCE_METADATA_MODEL_NAME: &str = "0x5265736f757263654d65746164617461";

pub const MANIFESTS_DIR: &str = "manifests";
pub const BASE_DIR: &str = "base";
pub const OVERLAYS_DIR: &str = "overlays";
pub const DEPLOYMENTS_DIR: &str = "deployments";
pub const ABIS_DIR: &str = "abis";

pub const CONTRACTS_DIR: &str = "contracts";
pub const MODELS_DIR: &str = "models";

#[derive(Error, Debug)]
pub enum AbstractManifestError {
    #[error("Remote World not found.")]
    RemoteWorldNotFound,
    #[error("Entry point name contains non-ASCII characters.")]
    InvalidEntryPointError,
    #[error(transparent)]
    CairoShortStringToFelt(#[from] CairoShortStringToFeltError),
    #[error(transparent)]
    ParseCairoShortString(#[from] ParseCairoShortStringError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    ContractRead(#[from] CainomeError),
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error(transparent)]
    TomlDe(#[from] toml::de::Error),
    #[error(transparent)]
    TomlSer(#[from] toml::ser::Error),
    #[error(transparent)]
    IO(#[from] io::Error),
    #[error("Abi couldn't be loaded from path: {0}")]
    AbiError(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl From<Manifest<Class>> for Manifest<WorldContract> {
    fn from(value: Manifest<Class>) -> Self {
        Manifest::new(
            WorldContract {
                class_hash: value.inner.class_hash,
                abi: value.inner.abi,
                original_class_hash: value.inner.original_class_hash,
                ..Default::default()
            },
            value.name,
        )
    }
}

impl From<BaseManifest> for DeploymentManifest {
    fn from(value: BaseManifest) -> Self {
        DeploymentManifest {
            world: value.world.into(),
            base: value.base,
            contracts: value.contracts,
            models: value.models,
        }
    }
}

impl BaseManifest {
    /// Load the manifest from a file at the given path.
    pub fn load_from_path(path: &Utf8PathBuf) -> Result<Self, AbstractManifestError> {
        let contract_dir = path.join(CONTRACTS_DIR);
        let model_dir = path.join(MODELS_DIR);

        let world: Manifest<Class> = toml::from_str(&fs::read_to_string(
            path.join(WORLD_CONTRACT_NAME.replace("::", "_")).with_extension("toml"),
        )?)?;

        let base: Manifest<Class> = toml::from_str(&fs::read_to_string(
            path.join(BASE_CONTRACT_NAME.replace("::", "_")).with_extension("toml"),
        )?)?;

        let contracts = elements_from_path::<DojoContract>(&contract_dir)?;
        let models = elements_from_path::<DojoModel>(&model_dir)?;

        Ok(Self { world, base, contracts, models })
    }

    /// Given a list of contract or model names, remove those from the manifest.
    pub fn remove_items(&mut self, items: Vec<String>) {
        self.contracts.retain(|contract| !items.contains(&contract.name.to_string()));
        self.models.retain(|model| !items.contains(&model.name.to_string()));
    }

    pub fn merge(&mut self, overlay: OverlayManifest) {
        let mut base_map = HashMap::new();

        for contract in self.contracts.iter_mut() {
            base_map.insert(contract.name.clone(), contract);
        }

        for contract in overlay.contracts {
            if let Some(manifest) = base_map.get_mut(&contract.name) {
                manifest.inner.merge(contract);
            } else {
                error!(
                    "OverlayManifest configured for contract \"{}\", but contract is not present \
                     in BaseManifest.",
                    contract.name
                );
            }
        }

        if let Some(overlay_world) = overlay.world {
            self.world.inner.merge(overlay_world);
        }
        if let Some(overlay_base) = overlay.base {
            self.base.inner.merge(overlay_base);
        }
    }
}

impl OverlayManifest {
    pub fn load_from_path(path: &Utf8PathBuf) -> Result<Self, AbstractManifestError> {
        fs::create_dir_all(path)?;

        let mut world: Option<OverlayClass> = None;

        let world_path = path.join(WORLD_CONTRACT_NAME.replace("::", "_")).with_extension("toml");

        if world_path.exists() {
            world = Some(toml::from_str(&fs::read_to_string(world_path)?)?);
        }

        let mut base: Option<OverlayClass> = None;
        let base_path = path.join(BASE_CONTRACT_NAME.replace("::", "_")).with_extension("toml");

        if base_path.exists() {
            base = Some(toml::from_str(&fs::read_to_string(base_path)?)?);
        }

        let contract_dir = path.join(CONTRACTS_DIR);
        let contracts = if contract_dir.exists() {
            overlay_elements_from_path::<OverlayDojoContract>(&contract_dir)?
        } else {
            vec![]
        };

        let model_dir = path.join(MODELS_DIR);
        let models = if model_dir.exists() {
            overlay_elements_from_path::<OverlayDojoModel>(&model_dir)?
        } else {
            vec![]
        };

        Ok(Self { world, base, contracts, models })
    }

    /// Writes `Self` to overlay manifests folder.
    ///
    /// - `world` and `base` manifest are written to root of the folder.
    /// - `contracts` and `models` are written to their respective directories.
    pub fn write_to_path_nested(&self, path: &Utf8PathBuf) -> Result<(), AbstractManifestError> {
        fs::create_dir_all(path)?;

        if let Some(ref world) = self.world {
            let world = toml::to_string(world)?;
            let file_name =
                path.join(WORLD_CONTRACT_NAME.replace("::", "_")).with_extension("toml");
            fs::write(file_name, world)?;
        }

        if let Some(ref base) = self.base {
            let base = toml::to_string(base)?;
            let file_name = path.join(BASE_CONTRACT_NAME.replace("::", "_")).with_extension("toml");
            fs::write(file_name, base)?;
        }

        overlay_dojo_contracts_to_path(&path.join(CONTRACTS_DIR), self.contracts.as_slice())?;
        overlay_dojo_model_to_path(&path.join(MODELS_DIR), self.models.as_slice())?;
        Ok(())
    }

    /// Add missing overlay items from `others` to `self`.
    /// Note that this method don't override if certain item already exists in `self`.
    pub fn merge(&mut self, other: OverlayManifest) {
        if self.world.is_none() {
            self.world = other.world;
        }

        if self.base.is_none() {
            self.base = other.base;
        }

        for other_contract in other.contracts {
            let found = self.contracts.iter().find(|c| c.name == other_contract.name);
            if found.is_none() {
                self.contracts.push(other_contract);
            }
        }

        for other_model in other.models {
            let found = self.models.iter().find(|m| m.name == other_model.name);
            if found.is_none() {
                self.models.push(other_model);
            }
        }
    }
}

impl DeploymentManifest {
    pub fn load_from_path(path: &Utf8PathBuf) -> Result<Self, AbstractManifestError> {
        let manifest: Self = toml::from_str(&fs::read_to_string(path)?).unwrap();

        Ok(manifest)
    }

    pub fn merge_from_previous(&mut self, previous: DeploymentManifest) {
        self.world.inner.transaction_hash = previous.world.inner.transaction_hash;
        self.world.inner.block_number = previous.world.inner.block_number;
        self.world.inner.seed = previous.world.inner.seed;

        self.contracts.iter_mut().for_each(|contract| {
            let previous_contract = previous.contracts.iter().find(|c| c.name == contract.name);
            if let Some(previous_contract) = previous_contract {
                if previous_contract.inner.base_class_hash != Felt::ZERO {
                    contract.inner.base_class_hash = previous_contract.inner.base_class_hash;
                }
            }
        });
    }

    pub fn write_to_path_toml(&self, path: &Utf8PathBuf) -> Result<()> {
        fs::create_dir_all(path.parent().unwrap())?;

        let deployed_manifest = toml::to_string_pretty(&self)?;
        fs::write(path, deployed_manifest)?;

        Ok(())
    }

    pub fn write_to_path_json(&self, path: &Utf8PathBuf, profile_dir: &Utf8PathBuf) -> Result<()> {
        fs::create_dir_all(path.parent().unwrap())?;

        // Embedding ABIs into the manifest.
        let mut manifest_with_abis = self.clone();

        if let Some(abi_format) = &manifest_with_abis.world.inner.abi {
            manifest_with_abis.world.inner.abi = Some(abi_format.to_embed(profile_dir)?);
        }

        for contract in &mut manifest_with_abis.contracts {
            if let Some(abi_format) = &contract.inner.abi {
                contract.inner.abi = Some(abi_format.to_embed(profile_dir)?);
            }
        }

        for model in &mut manifest_with_abis.models {
            if let Some(abi_format) = &model.inner.abi {
                model.inner.abi = Some(abi_format.to_embed(profile_dir)?);
            }
        }

        let deployed_manifest = serde_json::to_string_pretty(&manifest_with_abis)?;
        fs::write(path, deployed_manifest)?;

        Ok(())
    }

    /// Construct a manifest of a remote World.
    ///
    /// # Arguments
    /// * `provider` - A Starknet RPC provider.
    /// * `world_address` - The address of the remote World contract.
    pub async fn load_from_remote<P>(
        provider: P,
        world_address: Felt,
    ) -> Result<Self, AbstractManifestError>
    where
        P: Provider + Send + Sync,
    {
        const BLOCK_ID: BlockId = BlockId::Tag(BlockTag::Pending);

        let world_class_hash =
            provider.get_class_hash_at(BLOCK_ID, world_address).await.map_err(|err| match err {
                ProviderError::StarknetError(StarknetError::ContractNotFound) => {
                    AbstractManifestError::RemoteWorldNotFound
                }
                err => err.into(),
            })?;

        let world = WorldContractReader::new(world_address, provider);

        let base_class_hash = world.base().block_id(BLOCK_ID).call().await?;
        let base_class_hash = base_class_hash.into();

        let (models, contracts) =
            get_remote_models_and_contracts(world_address, &world.provider()).await?;

        Ok(DeploymentManifest {
            models,
            contracts,
            world: Manifest::new(
                WorldContract {
                    address: Some(world_address),
                    class_hash: world_class_hash,
                    ..Default::default()
                },
                WORLD_CONTRACT_NAME.into(),
            ),
            base: Manifest::new(
                Class {
                    class_hash: base_class_hash,
                    abi: None,
                    original_class_hash: base_class_hash,
                },
                BASE_CONTRACT_NAME.into(),
            ),
        })
    }
}

// TODO: currently implementing this method using trait is causing lifetime issue due to
// `async_trait` macro which is hard to debug. So moved it as a async method on type itself.
// #[async_trait]
// pub trait RemoteLoadable<P: Provider + Sync + Send + 'static> {
//     async fn load_from_remote(
//         provider: P,
//         world_address: FieldElement,
//     ) -> Result<DeploymentManifest, AbstractManifestError>;
// }

// #[async_trait]
// impl<P: Provider + Sync + Send + 'static> RemoteLoadable<P> for DeploymentManifest {}

async fn get_remote_models_and_contracts<P: Provider>(
    world: Felt,
    provider: P,
) -> Result<(Vec<Manifest<DojoModel>>, Vec<Manifest<DojoContract>>), AbstractManifestError>
where
    P: Provider + Send + Sync,
{
    let registered_models_event_name = starknet_keccak("ModelRegistered".as_bytes());
    let contract_deployed_event_name = starknet_keccak("ContractDeployed".as_bytes());
    let contract_upgraded_event_name = starknet_keccak("ContractUpgraded".as_bytes());

    let events = get_events(
        &provider,
        world,
        vec![vec![
            registered_models_event_name,
            contract_deployed_event_name,
            contract_upgraded_event_name,
        ]],
    )
    .await?;

    let mut registered_models_events = vec![];
    let mut contract_deployed_events = vec![];
    let mut contract_upgraded_events = vec![];

    for event in events {
        match event.keys.first() {
            Some(event_name) if *event_name == registered_models_event_name => {
                registered_models_events.push(event)
            }
            Some(event_name) if *event_name == contract_deployed_event_name => {
                contract_deployed_events.push(event)
            }
            Some(event_name) if *event_name == contract_upgraded_event_name => {
                contract_upgraded_events.push(event)
            }
            _ => {}
        }
    }

    let models = parse_models_events(registered_models_events);
    let mut contracts = parse_contracts_events(contract_deployed_events, contract_upgraded_events);

    // fetch contracts name
    for contract in &mut contracts {
        let name = match provider
            .call(
                FunctionCall {
                    calldata: vec![],
                    entry_point_selector: selector!("dojo_resource"),
                    contract_address: contract.inner.address.expect("qed; missing address"),
                },
                BlockId::Tag(BlockTag::Latest),
            )
            .await
        {
            Ok(res) => parse_cairo_short_string(&res[0])?.into(),

            Err(ProviderError::StarknetError(StarknetError::ContractError(_))) => SmolStr::from(""),

            Err(err) => return Err(err.into()),
        };

        contract.name = name;
    }

    Ok((models, contracts))
}

async fn get_events<P: Provider + Send + Sync>(
    provider: P,
    world: Felt,
    keys: Vec<Vec<Felt>>,
) -> Result<Vec<EmittedEvent>, ProviderError> {
    const DEFAULT_CHUNK_SIZE: u64 = 100;

    let mut events: Vec<EmittedEvent> = vec![];
    let mut continuation_token = None;

    let filter =
        EventFilter { to_block: None, from_block: None, address: Some(world), keys: Some(keys) };

    loop {
        let res =
            provider.get_events(filter.clone(), continuation_token, DEFAULT_CHUNK_SIZE).await?;

        continuation_token = res.continuation_token;
        events.extend(res.events);

        if continuation_token.is_none() {
            break;
        }
    }

    Ok(events)
}

fn parse_contracts_events(
    deployed: Vec<EmittedEvent>,
    upgraded: Vec<EmittedEvent>,
) -> Vec<Manifest<DojoContract>> {
    fn retain_only_latest_upgrade_events(events: Vec<EmittedEvent>) -> HashMap<Felt, Felt> {
        // addr -> (block_num, class_hash)
        let mut upgrades: HashMap<Felt, (u64, Felt)> = HashMap::new();

        events.into_iter().for_each(|event| {
            let mut data = event.data.into_iter();

            let block_num = event.block_number;
            let class_hash = data.next().expect("qed; missing class hash");
            let address = data.next().expect("qed; missing address");

            // Events that do not have a block number are ignored because we are unable to evaluate
            // whether the events happened before or after the latest event that has been processed.
            if let Some(num) = block_num {
                upgrades
                    .entry(address)
                    .and_modify(|(current_block, current_class_hash)| {
                        if *current_block < num {
                            *current_block = num;
                            *current_class_hash = class_hash;
                        }
                    })
                    .or_insert((num, class_hash));
            }
        });

        upgrades.into_iter().map(|(addr, (_, class_hash))| (addr, class_hash)).collect()
    }

    let upgradeds = retain_only_latest_upgrade_events(upgraded);

    deployed
        .into_iter()
        .map(|event| {
            let mut data = event.data.into_iter();

            let _ = data.next().expect("salt is missing from event");
            let mut class_hash = data.next().expect("class hash is missing from event");
            let address = data.next().expect("addresss is missing from event");

            if let Some(upgrade) = upgradeds.get(&address) {
                class_hash = *upgrade;
            }

            Manifest::new(
                DojoContract {
                    address: Some(address),
                    class_hash,
                    abi: None,
                    ..Default::default()
                },
                Default::default(),
            )
        })
        .collect()
}

fn parse_models_events(events: Vec<EmittedEvent>) -> Vec<Manifest<DojoModel>> {
    let mut models: HashMap<String, Felt> = HashMap::with_capacity(events.len());

    for e in events {
        let model_event = match e.try_into() {
            Ok(WorldEvent::ModelRegistered(mr)) => mr,
            Ok(_) => panic!("ModelRegistered expected as already filtered"),
            Err(_) => {
                // As models were registered with the new event type, we can
                // skip old ones. We are sure at least 1 new event was emitted
                // when models were migrated.
                continue;
            }
        };

        // TODO: Safely unwrap?
        let model_name = model_event.name.to_string().unwrap();
        if let Some(current_class_hash) = models.get_mut(&model_name) {
            if current_class_hash == &model_event.prev_class_hash.into() {
                *current_class_hash = model_event.class_hash.into();
            }
        } else {
            models.insert(model_name, model_event.class_hash.into());
        }
    }

    // TODO: include address of the model in the manifest.
    models
        .into_iter()
        .map(|(name, class_hash)| Manifest::<DojoModel> {
            inner: DojoModel { class_hash, abi: None, ..Default::default() },
            name: name.into(),
        })
        .collect()
}

fn elements_from_path<T>(path: &Utf8PathBuf) -> Result<Vec<Manifest<T>>, AbstractManifestError>
where
    T: DeserializeOwned + ManifestMethods,
{
    let mut elements = vec![];

    let mut entries = path
        .read_dir()?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()?;

    // `read_dir` doesn't guarantee any order, so we sort the entries ourself.
    // see: https://doc.rust-lang.org/std/fs/fn.read_dir.html#platform-specific-behavior
    entries.sort();

    for path in entries {
        if path.is_file() {
            let manifest: Manifest<T> = toml::from_str(&fs::read_to_string(path)?)?;
            elements.push(manifest);
        } else {
            continue;
        }
    }

    Ok(elements)
}

fn overlay_elements_from_path<T>(path: &Utf8PathBuf) -> Result<Vec<T>, AbstractManifestError>
where
    T: DeserializeOwned,
{
    let mut elements = vec![];

    for entry in path.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let manifest: T = toml::from_str(&fs::read_to_string(path)?)?;
            elements.push(manifest);
        } else {
            continue;
        }
    }

    Ok(elements)
}

fn overlay_dojo_contracts_to_path(
    path: &Utf8PathBuf,
    elements: &[OverlayDojoContract],
) -> Result<(), AbstractManifestError> {
    fs::create_dir_all(path)?;

    for element in elements {
        let path = path.join(element.name.replace("::", "_")).with_extension("toml");
        fs::write(path, toml::to_string(&element)?)?;
    }
    Ok(())
}

fn overlay_dojo_model_to_path(
    path: &Utf8PathBuf,
    elements: &[OverlayDojoModel],
) -> Result<(), AbstractManifestError> {
    fs::create_dir_all(path)?;

    for element in elements {
        let path = path.join(element.name.replace("::", "_")).with_extension("toml");
        fs::write(path, toml::to_string(&element)?)?;
    }
    Ok(())
}

impl ManifestMethods for DojoContract {
    type OverlayType = OverlayDojoContract;

    fn abi(&self) -> Option<&AbiFormat> {
        self.abi.as_ref()
    }

    fn set_abi(&mut self, abi: Option<AbiFormat>) {
        self.abi = abi;
    }

    fn class_hash(&self) -> &Felt {
        self.class_hash.as_ref()
    }

    fn set_class_hash(&mut self, class_hash: Felt) {
        self.class_hash = class_hash;
    }

    fn original_class_hash(&self) -> &Felt {
        self.original_class_hash.as_ref()
    }

    fn merge(&mut self, old: Self::OverlayType) {
        if let Some(class_hash) = old.original_class_hash {
            self.original_class_hash = class_hash;
        }
        if let Some(reads) = old.reads {
            self.reads = reads;
        }
        if let Some(writes) = old.writes {
            self.writes = writes;
        }
        if let Some(init_calldata) = old.init_calldata {
            self.init_calldata = init_calldata;
        }
    }
}

impl ManifestMethods for DojoModel {
    type OverlayType = OverlayDojoModel;

    fn abi(&self) -> Option<&AbiFormat> {
        self.abi.as_ref()
    }

    fn set_abi(&mut self, abi: Option<AbiFormat>) {
        self.abi = abi;
    }

    fn class_hash(&self) -> &Felt {
        self.class_hash.as_ref()
    }

    fn set_class_hash(&mut self, class_hash: Felt) {
        self.class_hash = class_hash;
    }

    fn original_class_hash(&self) -> &Felt {
        self.original_class_hash.as_ref()
    }

    fn merge(&mut self, old: Self::OverlayType) {
        if let Some(class_hash) = old.original_class_hash {
            self.original_class_hash = class_hash;
        }
    }
}

impl ManifestMethods for WorldContract {
    type OverlayType = OverlayContract;

    fn abi(&self) -> Option<&AbiFormat> {
        self.abi.as_ref()
    }

    fn set_abi(&mut self, abi: Option<AbiFormat>) {
        self.abi = abi;
    }

    fn class_hash(&self) -> &Felt {
        self.class_hash.as_ref()
    }

    fn set_class_hash(&mut self, class_hash: Felt) {
        self.class_hash = class_hash;
    }

    fn original_class_hash(&self) -> &Felt {
        self.original_class_hash.as_ref()
    }

    fn merge(&mut self, old: Self::OverlayType) {
        if let Some(class_hash) = old.original_class_hash {
            self.original_class_hash = class_hash;
        }
    }
}

impl ManifestMethods for Class {
    type OverlayType = OverlayClass;

    fn abi(&self) -> Option<&AbiFormat> {
        self.abi.as_ref()
    }

    fn set_abi(&mut self, abi: Option<AbiFormat>) {
        self.abi = abi;
    }

    fn class_hash(&self) -> &Felt {
        self.class_hash.as_ref()
    }

    fn set_class_hash(&mut self, class_hash: Felt) {
        self.class_hash = class_hash;
    }

    fn original_class_hash(&self) -> &Felt {
        self.original_class_hash.as_ref()
    }

    fn merge(&mut self, old: Self::OverlayType) {
        if let Some(class_hash) = old.original_class_hash {
            self.original_class_hash = class_hash;
        }
    }
}
