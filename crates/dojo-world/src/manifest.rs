use std::collections::HashMap;
use std::{fs, io};

use anyhow::Result;
use cainome::cairo_serde::Error as CainomeError;
use camino::Utf8PathBuf;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use smol_str::SmolStr;
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::{
    BlockId, BlockTag, EmittedEvent, EventFilter, FieldElement, FunctionCall, StarknetError,
};
use starknet::core::utils::{
    parse_cairo_short_string, starknet_keccak, CairoShortStringToFeltError,
    ParseCairoShortStringError,
};
use starknet::macros::selector;
use starknet::providers::{Provider, ProviderError};
use thiserror::Error;
use toml;

use crate::contracts::model::ModelError;
use crate::contracts::world::WorldEvent;
use crate::contracts::WorldContractReader;

#[cfg(test)]
#[path = "manifest_test.rs"]
mod test;

pub const WORLD_CONTRACT_NAME: &str = "dojo::world::world";
pub const BASE_CONTRACT_NAME: &str = "dojo::base::base";
pub const RESOURCE_METADATA_CONTRACT_NAME: &str = "dojo::resource_metadata::resource_metadata";
pub const RESOURCE_METADATA_MODEL_NAME: &str = "0x5265736f757263654d65746164617461";

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
    IO(#[from] io::Error),
}

/// Represents a model member.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Member {
    /// Name of the member.
    pub name: String,
    /// Type of the member.
    #[serde(rename = "type")]
    pub ty: String,
    pub key: bool,
}

impl From<dojo_types::schema::Member> for Member {
    fn from(m: dojo_types::schema::Member) -> Self {
        Self { name: m.name, ty: m.ty.name(), key: m.key }
    }
}

/// Represents a declaration of a model.
#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub struct DojoModel {
    pub members: Vec<Member>,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub abi: Option<Utf8PathBuf>,
}

/// System input ABI.
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Input {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// System Output ABI.
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Output {
    #[serde(rename = "type")]
    pub ty: String,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct ComputedValueEntrypoint {
    // Name of the contract containing the entrypoint
    pub contract: SmolStr,
    // Name of entrypoint to get computed value
    pub entrypoint: SmolStr,
    // Component to compute for
    pub model: Option<String>,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub struct DojoContract {
    #[serde_as(as = "Option<UfeHex>")]
    pub address: Option<FieldElement>,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub abi: Option<Utf8PathBuf>,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub computed: Vec<ComputedValueEntrypoint>,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]

pub struct OverlayDojoContract {
    pub name: SmolStr,
    pub reads: Option<Vec<String>>,
    pub writes: Option<Vec<String>>,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct OverlayDojoModel {}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct OverlayContract {}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct OverlayClass {}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub struct Class {
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub abi: Option<Utf8PathBuf>,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub struct Contract {
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub abi: Option<Utf8PathBuf>,
    #[serde_as(as = "Option<UfeHex>")]
    pub address: Option<FieldElement>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BaseManifest {
    pub world: Manifest<Class>,
    pub base: Manifest<Class>,
    pub contracts: Vec<Manifest<DojoContract>>,
    pub models: Vec<Manifest<DojoModel>>,
}

impl From<Manifest<Class>> for Manifest<Contract> {
    fn from(value: Manifest<Class>) -> Self {
        Manifest::new(
            Contract { class_hash: value.inner.class_hash, abi: value.inner.abi, address: None },
            value.name,
        )
    }
}

impl From<BaseManifest> for DeployedManifest {
    fn from(value: BaseManifest) -> Self {
        DeployedManifest {
            world: value.world.into(),
            base: value.base,
            contracts: value.contracts,
            models: value.models,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DeployedManifest {
    pub world: Manifest<Contract>,
    pub base: Manifest<Class>,
    pub contracts: Vec<Manifest<DojoContract>>,
    pub models: Vec<Manifest<DojoModel>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OverlayManifest {
    pub contracts: Vec<OverlayDojoContract>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Manifest<T>
where
    T: ManifestMethods,
{
    #[serde(flatten)]
    pub inner: T,
    pub name: SmolStr,
}

impl<T> Manifest<T>
where
    T: ManifestMethods,
{
    pub fn new(inner: T, name: SmolStr) -> Self {
        Self { inner, name }
    }
}

pub trait ManifestMethods {
    type OverlayType;
    fn abi(&self) -> Option<&Utf8PathBuf>;
    fn set_abi(&mut self, abi: Option<Utf8PathBuf>);
    fn class_hash(&self) -> &FieldElement;
    fn set_class_hash(&mut self, class_hash: FieldElement);

    /// This method is called when during compilation base manifest file already exists.
    /// Manifest generated during compilation won't contains properties manually updated by users
    /// (like calldata) so this method should override those fields
    fn merge(&mut self, old: Self::OverlayType);
}

impl BaseManifest {
    /// Load the manifest from a file at the given path.
    pub fn load_from_path(path: &Utf8PathBuf) -> Result<Self, AbstractManifestError> {
        let contract_dir = path.join("contracts");
        let model_dir = path.join("models");

        let world: Manifest<Class> =
            toml::from_str(&fs::read_to_string(path.join("world.toml"))?).unwrap();
        let base: Manifest<Class> =
            toml::from_str(&fs::read_to_string(path.join("base.toml"))?).unwrap();

        let contracts = elements_from_path::<DojoContract>(&contract_dir)?;
        let models = elements_from_path::<DojoModel>(&model_dir)?;

        Ok(Self { world, base, contracts, models })
    }

    pub fn merge(&mut self, overlay: OverlayManifest) {
        let mut base_map = HashMap::new();

        for contract in self.contracts.iter_mut() {
            base_map.insert(contract.name.clone(), contract);
        }

        for contract in overlay.contracts {
            base_map
                .get_mut(&contract.name)
                .expect("qed; overlay contract not found")
                .inner
                .merge(contract);
        }
    }
}

impl OverlayManifest {
    pub fn load_from_path(path: &Utf8PathBuf) -> Result<Self, AbstractManifestError> {
        let contract_dir = path.join("contracts");
        let contracts = overlay_elements_from_path::<OverlayDojoContract>(&contract_dir)?;

        Ok(Self { contracts })
    }
}

impl DeployedManifest {
    pub fn load_from_path(path: &Utf8PathBuf) -> Result<Self, AbstractManifestError> {
        let manifest: Self = toml::from_str(&fs::read_to_string(path)?).unwrap();

        Ok(manifest)
    }

    pub fn write_to_path(&self, path: &Utf8PathBuf) -> Result<()> {
        fs::create_dir_all(path.parent().unwrap())?;

        let deployed_manifest = toml::to_string_pretty(&self)?;
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
        world_address: FieldElement,
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

        let (models, contracts) =
            get_remote_models_and_contracts(world_address, &world.provider()).await?;

        Ok(DeployedManifest {
            models,
            contracts,
            world: Manifest::new(
                Contract { address: Some(world_address), class_hash: world_class_hash, abi: None },
                WORLD_CONTRACT_NAME.into(),
            ),
            base: Manifest::new(
                Class { class_hash: base_class_hash.into(), abi: None },
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
//     ) -> Result<DeployedManifest, AbstractManifestError>;
// }

// #[async_trait]
// impl<P: Provider + Sync + Send + 'static> RemoteLoadable<P> for DeployedManifest {}

async fn get_remote_models_and_contracts<P: Provider>(
    world: FieldElement,
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
    world: FieldElement,
    keys: Vec<Vec<FieldElement>>,
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
    fn retain_only_latest_upgrade_events(
        events: Vec<EmittedEvent>,
    ) -> HashMap<FieldElement, FieldElement> {
        // addr -> (block_num, class_hash)
        let mut upgrades: HashMap<FieldElement, (u64, FieldElement)> = HashMap::new();

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
    let mut models: HashMap<String, FieldElement> = HashMap::with_capacity(events.len());

    for e in events {
        let model_event = if let WorldEvent::ModelRegistered(m) =
            e.try_into().expect("ModelRegistered event is expected to be parseable")
        {
            m
        } else {
            panic!("ModelRegistered expected");
        };

        let model_name = parse_cairo_short_string(&model_event.name).unwrap();

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

// fn elements_to_path<T>(item_dir: &Utf8PathBuf, items: &Vec<Manifest<T>>) -> Result<()>
// where
//     T: Serialize + ManifestMethods,
// {
//     fs::create_dir_all(item_dir)?;
//     for item in items {
//         let item_toml = toml::to_string_pretty(&item)?;
//         let item_name = item.name.split("::").last().unwrap();
//         fs::write(item_dir.join(item_name).with_extension("toml"), item_toml)?;
//     }

//     Ok(())
// }

fn elements_from_path<T>(path: &Utf8PathBuf) -> Result<Vec<Manifest<T>>, AbstractManifestError>
where
    T: DeserializeOwned + ManifestMethods,
{
    let mut elements = vec![];

    for entry in path.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let manifest: Manifest<T> = toml::from_str(&fs::read_to_string(path)?).unwrap();
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
            let manifest: T = toml::from_str(&fs::read_to_string(path)?).unwrap();
            elements.push(manifest);
        } else {
            continue;
        }
    }

    Ok(elements)
}

impl ManifestMethods for DojoContract {
    type OverlayType = OverlayDojoContract;

    fn abi(&self) -> Option<&Utf8PathBuf> {
        self.abi.as_ref()
    }

    fn set_abi(&mut self, abi: Option<Utf8PathBuf>) {
        self.abi = abi;
    }

    fn class_hash(&self) -> &FieldElement {
        self.class_hash.as_ref()
    }

    fn set_class_hash(&mut self, class_hash: FieldElement) {
        self.class_hash = class_hash;
    }

    fn merge(&mut self, old: Self::OverlayType) {
        if let Some(reads) = old.reads {
            self.reads = reads;
        }
        if let Some(writes) = old.writes {
            self.writes = writes;
        }
    }
}

impl ManifestMethods for DojoModel {
    type OverlayType = OverlayDojoModel;

    fn abi(&self) -> Option<&Utf8PathBuf> {
        self.abi.as_ref()
    }

    fn set_abi(&mut self, abi: Option<Utf8PathBuf>) {
        self.abi = abi;
    }

    fn class_hash(&self) -> &FieldElement {
        self.class_hash.as_ref()
    }

    fn set_class_hash(&mut self, class_hash: FieldElement) {
        self.class_hash = class_hash;
    }

    fn merge(&mut self, _: Self::OverlayType) {}
}

impl ManifestMethods for Contract {
    type OverlayType = OverlayContract;

    fn abi(&self) -> Option<&Utf8PathBuf> {
        self.abi.as_ref()
    }

    fn set_abi(&mut self, abi: Option<Utf8PathBuf>) {
        self.abi = abi;
    }

    fn class_hash(&self) -> &FieldElement {
        self.class_hash.as_ref()
    }

    fn set_class_hash(&mut self, class_hash: FieldElement) {
        self.class_hash = class_hash;
    }

    fn merge(&mut self, _: Self::OverlayType) {}
}

impl ManifestMethods for Class {
    type OverlayType = OverlayClass;

    fn abi(&self) -> Option<&Utf8PathBuf> {
        self.abi.as_ref()
    }

    fn set_abi(&mut self, abi: Option<Utf8PathBuf>) {
        self.abi = abi;
    }

    fn class_hash(&self) -> &FieldElement {
        self.class_hash.as_ref()
    }

    fn set_class_hash(&mut self, class_hash: FieldElement) {
        self.class_hash = class_hash;
    }

    fn merge(&mut self, _: Self::OverlayType) {}
}
