use std::collections::HashMap;

use ::serde::{Deserialize, Serialize};
use async_trait::async_trait;
use cainome::cairo_serde::Error as CainomeError;
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

use crate::contracts::model::ModelError;
use crate::contracts::WorldContractReader;

#[cfg(test)]
#[path = "manifest_test.rs"]
mod test;

pub const WORLD_CONTRACT_NAME: &str = "dojo::world::world";
pub const EXECUTOR_CONTRACT_NAME: &str = "dojo::executor::executor";
pub const BASE_CONTRACT_NAME: &str = "dojo::base::base";

#[derive(Error, Debug)]
pub enum WorldError {
    #[error("Remote World not found.")]
    RemoteWorldNotFound,
    #[error("Executor contract not found.")]
    ExecutorNotFound,
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
pub struct Model {
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub abi: Option<String>,
    pub members: Vec<Member>,
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
pub struct Contract {
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub abi: Option<String>,
    #[serde_as(as = "Option<UfeHex>")]
    pub address: Option<FieldElement>,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub computed: Vec<ComputedValueEntrypoint>,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Class {
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub abi: Option<String>,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Manifest {
    pub kind: ManifestKind,
    pub name: SmolStr,
}

impl Manifest {
    pub fn class_hash(&self) -> FieldElement {
        match &self.kind {
            ManifestKind::Class(class) => class.class_hash,
            ManifestKind::Contract(contract) => contract.class_hash,
            ManifestKind::Model(model) => model.class_hash,
        }
    }

    pub fn abi(&self) -> Option<&String> {
        match &self.kind {
            ManifestKind::Class(class) => class.abi.as_ref(),
            ManifestKind::Contract(contract) => contract.abi.as_ref(),
            ManifestKind::Model(model) => model.abi.as_ref(),
        }
    }

    pub fn set_class_hash(&mut self, class_hash: FieldElement) {
        match &mut self.kind {
            ManifestKind::Class(class) => class.class_hash = class_hash,
            ManifestKind::Contract(contract) => contract.class_hash = class_hash,
            ManifestKind::Model(model) => model.class_hash = class_hash,
        }
    }

    pub fn set_abi(&mut self, abi: Option<String>) {
        match &mut self.kind {
            ManifestKind::Class(class) => class.abi = abi,
            ManifestKind::Contract(contract) => contract.abi = abi,
            ManifestKind::Model(model) => model.abi = abi,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ManifestKind {
    Class(Class),
    Contract(Contract),
    Model(Model),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct World {
    pub world: Manifest,
    pub executor: Manifest,
    pub base: Manifest,
    pub contracts: Vec<Manifest>,
    pub models: Vec<Manifest>,
}

// impl World {
//     pub fn load_deployed_from_path(path: Utf8PathBuf) -> Result<Self, std::io::Error> {
//         Ok(World {
//             world: todo!(),
//             executor: todo!(),
//             base: todo!(),
//             contracts: todo!(),
//             models: todo!(),
//         })
//     }

// pub fn load_base_from_path(path: Utf8PathBuf) -> Result<Self, std::io::Error> {
//     let base_dir = Utf8PathBuf::new().join(path).join("manifest").join("base");
//     let contract_dir =
//         Utf8PathBuf::new().join(path).join("manifest").join("base").join("contracts");
//     let model_dir =
//         Utf8PathBuf::new().join(path).join("manifest").join("base").join("contracts");

//     let world: Manifest =
//         toml::from_str(&fs::read_to_string(base_dir.join("world.toml"))?).unwrap();
//     let executor: Manifest =
//         toml::from_str(&fs::read_to_string(base_dir.join("executor.toml"))?).unwrap();
//     let base: Manifest =
//         toml::from_str(&fs::read_to_string(base_dir.join("base.toml"))?).unwrap();

//     contract_dir.iter()

//     Ok(World { world, executor, base, contracts: todo!(), models: todo!() })
// }

// Writes the manifest into a file at the given path. Will return error if the file doesn't
// exist.
// pub fn write_to_path(self, path: impl AsRef<Path>) -> Result<(), std::io::Error> {
//     let fd = fs::File::options().write(true).open(path)?;
//     Ok(serde_json::to_writer_pretty(fd, &self)?)
// }
// }

#[async_trait]
pub trait RemoteLoadable<P: Provider + Sync + Send + 'static> {
    async fn load_from_remote(
        provider: P,
        world_address: FieldElement,
    ) -> Result<World, WorldError>;
}

#[async_trait]
impl<P: Provider + Sync + Send + 'static> RemoteLoadable<P> for World {
    async fn load_from_remote(
        provider: P,
        world_address: FieldElement,
    ) -> Result<World, WorldError> {
        const BLOCK_ID: BlockId = BlockId::Tag(BlockTag::Pending);

        let world_class_hash =
            provider.get_class_hash_at(BLOCK_ID, world_address).await.map_err(|err| match err {
                ProviderError::StarknetError(StarknetError::ContractNotFound) => {
                    WorldError::RemoteWorldNotFound
                }
                err => err.into(),
            })?;
        let world = WorldContractReader::new(world_address, provider);
        let executor_address = world.executor().block_id(BLOCK_ID).call().await?;
        let base_class_hash = world.base().block_id(BLOCK_ID).call().await?;
        let executor_class_hash = world
            .provider()
            .get_class_hash_at(BLOCK_ID, FieldElement::from(executor_address))
            .await
            .map_err(|err| match err {
                ProviderError::StarknetError(StarknetError::ContractNotFound) => {
                    WorldError::ExecutorNotFound
                }
                err => err.into(),
            })?;

        let (models, contracts) =
            get_remote_models_and_contracts(world_address, &world.provider()).await?;

        // Err(WorldError::RemoteWorldNotFound)
        Ok(World {
            models,
            contracts,
            world: Manifest {
                name: WORLD_CONTRACT_NAME.into(),
                kind: ManifestKind::Contract(Contract {
                    address: Some(world_address),
                    class_hash: world_class_hash,
                    abi: None,
                    ..Default::default()
                }),
            },
            executor: Manifest {
                name: EXECUTOR_CONTRACT_NAME.into(),
                kind: ManifestKind::Contract(Contract {
                    address: Some(executor_address.into()),
                    class_hash: executor_class_hash,
                    abi: None,
                    ..Default::default()
                }),
            },
            base: Manifest {
                name: BASE_CONTRACT_NAME.into(),
                kind: ManifestKind::Class(Class { class_hash: base_class_hash.into(), abi: None }),
            },
        })
    }
}

async fn get_remote_models_and_contracts<P: Provider + Send + Sync>(
    world: FieldElement,
    provider: P,
) -> Result<(Vec<Manifest>, Vec<Manifest>), WorldError>
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
        let ManifestKind::Contract(ref inner) = contract.kind else {
            unreachable!("we only pass expected kind of manifest");
        };

        let name = match provider
            .call(
                FunctionCall {
                    calldata: vec![],
                    entry_point_selector: selector!("dojo_resource"),
                    contract_address: inner.address.expect("qed; missing address"),
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
) -> Vec<Manifest> {
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

            upgrades
                .entry(address)
                .and_modify(|(current_block, current_class_hash)| {
                    if *current_block < block_num {
                        *current_block = block_num;
                        *current_class_hash = class_hash;
                    }
                })
                .or_insert((block_num, class_hash));
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
            Manifest {
                kind: ManifestKind::Contract(Contract {
                    address: Some(address),
                    class_hash,
                    abi: None,
                    ..Default::default()
                }),
                name: Default::default(),
            }
        })
        .collect()
}

fn parse_models_events(events: Vec<EmittedEvent>) -> Vec<Manifest> {
    let mut models: HashMap<String, FieldElement> = HashMap::with_capacity(events.len());

    for event in events {
        let mut data = event.data.into_iter();

        let model_name = data.next().expect("name is missing from event");
        let model_name = parse_cairo_short_string(&model_name).unwrap();

        let class_hash = data.next().expect("class hash is missing from event");
        let prev_class_hash = data.next().expect("prev class hash is missing from event");

        if let Some(current_class_hash) = models.get_mut(&model_name) {
            if current_class_hash == &prev_class_hash {
                *current_class_hash = class_hash;
            }
        } else {
            models.insert(model_name, class_hash);
        }
    }

    models
        .into_iter()
        .map(|(name, class_hash)| Manifest {
            kind: ManifestKind::Model(Model { class_hash, abi: None, ..Default::default() }),
            name: name.into(),
        })
        .collect()
}
