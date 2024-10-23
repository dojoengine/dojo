use std::collections::HashMap;
use std::path::PathBuf;
use std::{fs, io};

use anyhow::Result;
use cainome::cairo_serde::{ByteArray, CairoSerde, Error as CainomeError, Zeroable};
use camino::Utf8PathBuf;
use serde::de::DeserializeOwned;
use serde::Serialize;
use starknet::core::types::{BlockId, BlockTag, EmittedEvent, EventFilter, Felt, StarknetError};
use starknet::core::utils::{
    starknet_keccak, CairoShortStringToFeltError, ParseCairoShortStringError,
};
use starknet::providers::{Provider, ProviderError};
use thiserror::Error;
use toml;
use toml::Table;
use tracing::error;
use walkdir::WalkDir;

use crate::contracts::model::ModelError;
use crate::contracts::world::WorldEvent;
use crate::contracts::{naming, WorldContractReader};

#[cfg(test)]
#[path = "manifest_test.rs"]
mod test;

mod types;

pub use types::{
    AbiFormat, BaseManifest, Class, DeploymentManifest, DojoContract, DojoModel, Manifest,
    ManifestMethods, Member, OverlayClass, OverlayContract, OverlayDojoContract, OverlayDojoModel,
    OverlayManifest, WorldContract, WorldMetadata,
};

pub const WORLD_CONTRACT_TAG: &str = "dojo-world";
pub const BASE_CONTRACT_TAG: &str = "dojo-base";

pub const WORLD_QUALIFIED_PATH: &str = "dojo::world::world_contract::world";
pub const BASE_QUALIFIED_PATH: &str = "dojo::contract::base_contract::base";

pub const MANIFESTS_DIR: &str = "manifests";
pub const DEPLOYMENT_DIR: &str = "deployment";
pub const TARGET_DIR: &str = "target";
pub const BASE_DIR: &str = "base";
pub const OVERLAYS_DIR: &str = "overlays";
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
    #[error("Duplicated manifest : {0}")]
    DuplicatedManifest(String),
    #[error("{0}")]
    TagError(String),
    #[error("{0}")]
    UnknownTarget(String),
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
            value.manifest_name,
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
        let world: Manifest<Class> = toml::from_str(&fs::read_to_string(
            path.join(naming::get_filename_from_tag(WORLD_CONTRACT_TAG)).with_extension("toml"),
        )?)?;

        let base: Manifest<Class> = toml::from_str(&fs::read_to_string(
            path.join(naming::get_filename_from_tag(BASE_CONTRACT_TAG)).with_extension("toml"),
        )?)?;

        let contracts = elements_from_path::<DojoContract>(&path.join(CONTRACTS_DIR))?;
        let models = elements_from_path::<DojoModel>(&path.join(MODELS_DIR))?;

        Ok(Self { world, base, contracts, models })
    }

    /// Given a list of contract or model tags, remove those from the manifest.
    pub fn remove_tags(&mut self, tags: &[String]) {
        self.contracts.retain(|contract| !tags.contains(&contract.inner.tag));
        self.models.retain(|model| !tags.contains(&model.inner.tag));
    }

    /// Generates a map of `tag -> ManifestKind`
    pub fn build_kind_from_tags(&self) -> HashMap<String, ManifestKind> {
        let mut kind_from_tags = HashMap::<String, ManifestKind>::new();

        kind_from_tags.insert(WORLD_CONTRACT_TAG.to_string(), ManifestKind::WorldClass);
        kind_from_tags.insert(BASE_CONTRACT_TAG.to_string(), ManifestKind::BaseClass);

        for model in self.models.as_slice() {
            kind_from_tags.insert(model.inner.tag.clone(), ManifestKind::Model);
        }

        for contract in self.contracts.as_slice() {
            kind_from_tags.insert(contract.inner.tag.clone(), ManifestKind::Contract);
        }

        kind_from_tags
    }

    pub fn merge(&mut self, overlay: OverlayManifest) {
        let mut base_map = HashMap::new();

        for contract in self.contracts.iter_mut() {
            base_map.insert(contract.inner.tag.clone(), contract);
        }

        for contract in overlay.contracts {
            if let Some(manifest) = base_map.get_mut(&contract.tag) {
                manifest.inner.merge(contract);
            } else {
                error!(
                    "OverlayManifest configured for contract \"{}\", but contract is not present \
                     in BaseManifest.",
                    contract.tag
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

#[derive(Clone, Debug, Copy)]
pub enum ManifestKind {
    BaseClass,
    WorldClass,
    Contract,
    Model,
}

impl OverlayManifest {
    fn load_overlay(
        path: &PathBuf,
        kind: ManifestKind,
        overlays: &mut OverlayManifest,
    ) -> Result<(), AbstractManifestError> {
        match kind {
            ManifestKind::BaseClass => {
                let overlay: OverlayClass = toml::from_str(&fs::read_to_string(path)?)?;
                overlays.base = Some(overlay);
            }
            ManifestKind::WorldClass => {
                let overlay: OverlayClass = toml::from_str(&fs::read_to_string(path)?)?;
                overlays.world = Some(overlay);
            }
            ManifestKind::Model => {
                let overlay: OverlayDojoModel = toml::from_str(&fs::read_to_string(path)?)?;
                overlays.models.push(overlay);
            }
            ManifestKind::Contract => {
                let overlay: OverlayDojoContract = toml::from_str(&fs::read_to_string(path)?)?;
                overlays.contracts.push(overlay);
            }
        };

        Ok(())
    }

    pub fn load_from_path(
        path: &Utf8PathBuf,
        base_manifest: &BaseManifest,
    ) -> Result<Self, AbstractManifestError> {
        fs::create_dir_all(path)?;

        let kind_from_tags = base_manifest.build_kind_from_tags();
        let mut loaded_tags = HashMap::<String, bool>::new();
        let mut overlays = OverlayManifest::default();

        for entry in WalkDir::new(path).into_iter() {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => return Err(AbstractManifestError::IO(e.into())),
            };
            let file_path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            if !file_name.clone().ends_with(".toml") {
                continue;
            }

            // an overlay file must contain a 'tag' key.
            let toml_data = toml::from_str::<Table>(&fs::read_to_string(file_path)?)?;
            if !toml_data.contains_key("tag") {
                return Err(AbstractManifestError::TagError(format!(
                    "The overlay '{file_name}' must contain the 'tag' key."
                )));
            }

            // the tag key must be a string
            let tag = match toml_data["tag"].as_str() {
                Some(x) => x.to_string(),
                None => {
                    return Err(AbstractManifestError::TagError(format!(
                        "The tag key of the overlay '{file_name}' must be a string."
                    )));
                }
            };

            // an overlay must target an existing class/model/contract
            if !kind_from_tags.contains_key(&tag) {
                return Err(AbstractManifestError::UnknownTarget(format!(
                    "The tag '{tag}' of the overlay '{file_name}' does not target an existing \
                     class/model/contract."
                )));
            }

            // a same tag cannot be used in multiple overlays.
            if loaded_tags.contains_key(&tag) {
                return Err(AbstractManifestError::DuplicatedManifest(format!(
                    "The tag '{tag}' is used in multiple overlays."
                )));
            }

            Self::load_overlay(&file_path.to_path_buf(), kind_from_tags[&tag], &mut overlays)?;
            loaded_tags.insert(tag, true);
        }

        Ok(overlays)
    }

    /// Writes `Self` to overlay manifests folder.
    ///
    /// - `world` and `base` manifest are written to root of the folder.
    /// - `contracts` and `models` are written to their respective directories.
    pub fn write_to_path(&self, path: &Utf8PathBuf) -> Result<(), AbstractManifestError> {
        fs::create_dir_all(path)?;

        if let Some(ref world) = self.world {
            let world = toml::to_string(world)?;
            let file_name =
                path.join(naming::get_filename_from_tag(WORLD_CONTRACT_TAG)).with_extension("toml");
            fs::write(file_name, world)?;
        }

        if let Some(ref base) = self.base {
            let base = toml::to_string(base)?;
            let file_name =
                path.join(naming::get_filename_from_tag(BASE_CONTRACT_TAG)).with_extension("toml");
            fs::write(file_name, base)?;
        }

        overlay_to_path::<OverlayDojoContract>(path, self.contracts.as_slice(), |c| c.tag.clone())?;
        overlay_to_path::<OverlayDojoModel>(path, self.models.as_slice(), |m| m.tag.clone())?;
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
            let found = self.contracts.iter().find(|c| c.tag == other_contract.tag);
            if found.is_none() {
                self.contracts.push(other_contract);
            }
        }

        for other_model in other.models {
            let found = self.models.iter().find(|m| m.tag == other_model.tag);
            if found.is_none() {
                self.models.push(other_model);
            }
        }
    }
}

impl DeploymentManifest {
    pub fn load_from_path(path: &Utf8PathBuf) -> Result<Self, AbstractManifestError> {
        let manifest: Self = toml::from_str(&fs::read_to_string(path)?)?;
        Ok(manifest)
    }

    pub fn merge_from_previous(&mut self, previous: DeploymentManifest) {
        self.world.inner.transaction_hash = previous.world.inner.transaction_hash;
        self.world.inner.block_number = previous.world.inner.block_number;
        self.world.inner.seed = previous.world.inner.seed;

        self.contracts.iter_mut().for_each(|contract| {
            let previous_contract =
                previous.contracts.iter().find(|c| c.manifest_name == contract.manifest_name);
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

    // Writes the Deployment manifest in JSON format, with ABIs embedded.
    pub fn write_to_path_json(&self, path: &Utf8PathBuf, root_dir: &Utf8PathBuf) -> Result<()> {
        fs::create_dir_all(path.parent().unwrap())?;

        // Embedding ABIs into the manifest.
        let mut manifest_with_abis = self.clone();

        if let Some(abi_format) = &manifest_with_abis.world.inner.abi {
            manifest_with_abis.world.inner.abi = Some(abi_format.to_embed(root_dir)?);
        }

        for contract in &mut manifest_with_abis.contracts {
            if let Some(abi_format) = &contract.inner.abi {
                contract.inner.abi = Some(abi_format.to_embed(root_dir)?);
            }
        }

        for model in &mut manifest_with_abis.models {
            if let Some(abi_format) = &model.inner.abi {
                model.inner.abi = Some(abi_format.to_embed(root_dir)?);
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
                naming::get_filename_from_tag(WORLD_CONTRACT_TAG),
            ),
            base: Manifest::new(
                Class {
                    class_hash: world_class_hash,
                    abi: None,
                    original_class_hash: world_class_hash,
                    tag: BASE_CONTRACT_TAG.to_string(),
                },
                naming::get_filename_from_tag(BASE_CONTRACT_TAG),
            ),
        })
    }
}

// impl DeploymentMetadata {
//     pub fn load_from_path(path: &Utf8PathBuf) -> Result<Self, AbstractManifestError> {
//         let manifest: Self = toml::from_str(&fs::read_to_string(path)?).unwrap();

//         Ok(manifest)
//     }

//     pub fn write_to_path_toml(&self, path: &Utf8PathBuf) -> Result<()> {
//         fs::create_dir_all(path.parent().unwrap())?;

//         let deployed_manifest = toml::to_string_pretty(&self)?;
//         fs::write(path, deployed_manifest)?;

//         Ok(())
//     }
// }

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

async fn get_remote_models_and_contracts<P>(
    world: Felt,
    provider: P,
) -> Result<(Vec<Manifest<DojoModel>>, Vec<Manifest<DojoContract>>), AbstractManifestError>
where
    P: Provider + Send + Sync,
{
    let registered_models_event_name = starknet_keccak("ModelRegistered".as_bytes());
    let contract_deployed_event_name = starknet_keccak("ContractDeployed".as_bytes());
    let contract_upgraded_event_name = starknet_keccak("ContractUpgraded".as_bytes());
    let writer_updated_event_name = starknet_keccak("WriterUpdated".as_bytes());

    let events = get_events(
        &provider,
        world,
        vec![vec![
            registered_models_event_name,
            contract_deployed_event_name,
            contract_upgraded_event_name,
            writer_updated_event_name,
        ]],
    )
    .await?;

    let mut registered_models_events = vec![];
    let mut contract_deployed_events = vec![];
    let mut contract_upgraded_events = vec![];
    let mut writer_updated_events = vec![];

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
            Some(event_name) if *event_name == writer_updated_event_name => {
                writer_updated_events.push(event)
            }
            _ => {}
        }
    }

    let models = parse_models_events(registered_models_events);
    let mut contracts = parse_contracts_events(
        contract_deployed_events,
        contract_upgraded_events,
        writer_updated_events,
    );

    for contract in &mut contracts {
        contract.manifest_name = naming::get_filename_from_tag(&contract.inner.tag);
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

        // stop when there are no more events being returned
        if res.events.is_empty() {
            break;
        } else {
            events.extend(res.events);
        }

        if continuation_token.is_none() {
            break;
        }
    }

    Ok(events)
}

fn parse_contracts_events(
    deployed: Vec<EmittedEvent>,
    upgraded: Vec<EmittedEvent>,
    granted: Vec<EmittedEvent>,
) -> Vec<Manifest<DojoContract>> {
    fn retain_only_latest_grant_events(events: Vec<EmittedEvent>) -> HashMap<Felt, Vec<Felt>> {
        // create a map with some extra data which will be flattened later
        // system -> (block_num, (resource -> perm))
        let mut grants: HashMap<Felt, (u64, HashMap<Felt, bool>)> = HashMap::new();
        events.into_iter().for_each(|event| {
            let mut data = event.data.into_iter();
            let block_num = event.block_number;
            let resource = data.next().expect("resource is missing from event");
            let contract = data.next().expect("contract is missing from event");
            let value = data.next().expect("value is missing from event");

            let value = !value.is_zero();

            // Events that do not have a block number are ignored because we are unable to evaluate
            // whether the events happened before or after the latest event that has been processed.
            if let Some(num) = block_num {
                grants
                    .entry(contract)
                    .and_modify(|(current_block, current_resource)| {
                        if *current_block <= num {
                            *current_block = num;
                            current_resource.insert(resource, value);
                        }
                    })
                    .or_insert((num, HashMap::from([(resource, value)])));
            }
        });

        // flatten out the map to remove block_number information and only include resources that
        // are true i.e. system -> [resources]
        grants
            .into_iter()
            .map(|(contract, (_, resources))| {
                (
                    contract,
                    resources
                        .into_iter()
                        .filter_map(|(resource, bool)| if bool { Some(resource) } else { None })
                        .collect(),
                )
            })
            .collect()
    }

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
                        if *current_block <= num {
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
    let grants = retain_only_latest_grant_events(granted);

    deployed
        .into_iter()
        .map(|event| {
            let mut data = event.data.into_iter();

            let _ = data.next().expect("salt is missing from event");
            let mut class_hash = data.next().expect("class hash is missing from event");
            let address = data.next().expect("addresss is missing from event");

            let str_data = data.as_slice();
            let namespace =
                ByteArray::cairo_deserialize(str_data, 0).expect("namespace is missing from event");
            let offset = ByteArray::cairo_serialized_size(&namespace);
            let name =
                ByteArray::cairo_deserialize(str_data, offset).expect("name is missing from event");

            let tag = naming::get_tag(
                &namespace.to_string().expect("ASCII encoded namespace"),
                &name.to_string().expect("ASCII encoded name"),
            );

            if let Some(upgrade) = upgradeds.get(&address) {
                class_hash = *upgrade;
            }

            let mut writes = vec![];
            if let Some(contract) = grants.get(&address) {
                writes.extend(contract.iter().map(|f| f.to_hex_string()));
            }

            Manifest::new(
                DojoContract {
                    address: Some(address),
                    class_hash,
                    abi: None,
                    tag: tag.clone(),
                    writes,
                    ..Default::default()
                },
                naming::get_filename_from_tag(&tag),
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

        let model_name = model_event.name.to_string().expect("ASCII encoded name");
        let namespace = model_event.namespace.to_string().expect("ASCII encoded namespace");
        let model_tag = naming::get_tag(&namespace, &model_name);

        models.insert(model_tag, model_event.class_hash.into());
    }

    // TODO: include address of the model in the manifest.
    models
        .into_iter()
        .map(|(tag, class_hash)| Manifest::<DojoModel> {
            inner: DojoModel { tag: tag.clone(), class_hash, abi: None, ..Default::default() },
            manifest_name: naming::get_filename_from_tag(&tag),
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

fn overlay_to_path<T>(
    path: &Utf8PathBuf,
    elements: &[T],
    get_tag: fn(&T) -> String,
) -> Result<(), AbstractManifestError>
where
    T: Serialize,
{
    fs::create_dir_all(path)?;

    for element in elements {
        let filename = naming::get_filename_from_tag(&get_tag(element));
        let path = path.join(filename).with_extension("toml");
        fs::write(path, toml::to_string(element)?)?;
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
        // ignore name and namespace

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
