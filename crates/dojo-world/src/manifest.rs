use std::collections::HashMap;

use ::serde::{Deserialize, Serialize};
use cainome::cairo_serde::Error as CainomeError;
use cairo_lang_starknet::abi;
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
use async_trait::async_trait;

use crate::contracts::model::ModelError;

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
    pub members: Vec<Member>,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub abi: Option<abi::Contract>,
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
    #[serde_as(as = "Option<UfeHex>")]
    pub address: Option<FieldElement>,
    pub abi: Option<abi::Contract>,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub computed: Vec<ComputedValueEntrypoint>,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Class {
    pub abi: Option<abi::Contract>,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Manifest {
    pub kind: ManifestKind,
    pub name: SmolStr,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
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

// we wont be writing the whole `World` to file so these are no longer required
// impl World {
//     /// Load the manifest from a file at the given path.
//     pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
//         let file = fs::File::open(path)?;
//         Ok(Self::try_from(file)?)
//     }

//     /// Writes the manifest into a file at the given path. Will return error if the file doesn't
//     /// exist.
//     pub fn write_to_path(self, path: impl AsRef<Path>) -> Result<(), std::io::Error> {
//         let fd = fs::File::options().write(true).open(path)?;
//         Ok(serde_json::to_writer_pretty(fd, &self)?)
//     }
// }

// impl TryFrom<std::fs::File> for World {
//     type Error = serde_json::Error;
//     fn try_from(file: std::fs::File) -> Result<Self, Self::Error> {
//         serde_json::from_reader(std::io::BufReader::new(file))
//     }
// }

// impl TryFrom<&std::fs::File> for World {
//     type Error = serde_json::Error;
//     fn try_from(file: &std::fs::File) -> Result<Self, Self::Error> {
//         serde_json::from_reader(std::io::BufReader::new(file))
//     }
// }

#[async_trait]
pub trait RemoteLoadable<P: Provider + Sync + Send> {
    async fn load_from_remote(
        provider: P,
        world_address: FieldElement,
    ) -> Result<World, WorldError>;
}

#[async_trait]
impl<P: Provider + Sync + Send> RemoteLoadable<P> for World {
    async fn load_from_remote(
        provider: P,
        world_address: FieldElement,
    ) -> Result<World, WorldError> {
        todo!();
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
                    ..Default::default()
                }),
                class_hash,
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
            kind: ManifestKind::Model(Model { ..Default::default() }),
            name: name.into(),
            class_hash,
        })
        .collect()
}
