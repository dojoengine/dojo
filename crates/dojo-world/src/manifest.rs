use std::collections::HashMap;
use std::fs;
use std::path::Path;

use ::serde::{Deserialize, Serialize};
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
use starknet::providers::{
    MaybeUnknownErrorCode, Provider, ProviderError, StarknetErrorWithMessage,
};
use thiserror::Error;

use crate::contracts::model::ModelError;
use crate::contracts::world::ContractReaderError;
use crate::contracts::WorldContractReader;

#[cfg(test)]
#[path = "manifest_test.rs"]
mod test;

pub const WORLD_CONTRACT_NAME: &str = "world";
pub const EXECUTOR_CONTRACT_NAME: &str = "executor";
pub const BASE_CONTRACT_NAME: &str = "base";

#[derive(Error, Debug)]
pub enum ManifestError<E> {
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
    Provider(#[from] ProviderError<E>),
    #[error(transparent)]
    ContractRead(#[from] ContractReaderError<E>),
    #[error(transparent)]
    Model(#[from] ModelError<E>),
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
    pub name: String,
    pub members: Vec<Member>,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub abi: Option<abi::Contract>,
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
pub struct Contract {
    pub name: SmolStr,
    #[serde_as(as = "Option<UfeHex>")]
    pub address: Option<FieldElement>,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub abi: Option<abi::Contract>,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Class {
    pub name: SmolStr,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub abi: Option<abi::Contract>,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub world: Contract,
    pub executor: Contract,
    pub base: Class,
    pub contracts: Vec<Contract>,
    pub models: Vec<Model>,
}

impl Manifest {
    /// Load the manifest from a file at the given path.
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let file = fs::File::open(path)?;
        Ok(serde_json::from_reader(file)?)
    }

    /// Writes the manifest into a file at the given path. Will return error if the file doesn't
    /// exist.
    pub fn write_to_path(self, path: impl AsRef<Path>) -> Result<(), std::io::Error> {
        let fd = fs::File::options().write(true).open(path)?;
        Ok(serde_json::to_writer_pretty(fd, &self)?)
    }

    /// Construct a manifest of a remote World.
    ///
    /// # Arguments
    /// * `provider` - A Starknet RPC provider.
    /// * `world_address` - The address of the remote World contract.
    pub async fn load_from_remote<P>(
        provider: P,
        world_address: FieldElement,
    ) -> Result<Self, ManifestError<<P as Provider>::Error>>
    where
        P::Error: 'static,
        P: Provider + Send + Sync,
    {
        const BLOCK_ID: BlockId = BlockId::Tag(BlockTag::Pending);

        let world_class_hash =
            provider.get_class_hash_at(BLOCK_ID, world_address).await.map_err(|err| match err {
                ProviderError::StarknetError(StarknetErrorWithMessage {
                    code: MaybeUnknownErrorCode::Known(StarknetError::ContractNotFound),
                    ..
                }) => ManifestError::RemoteWorldNotFound,
                err => err.into(),
            })?;

        let world = WorldContractReader::new(world_address, &provider).with_block(BLOCK_ID);

        let executor_address = world.executor().await?;
        let base_class_hash = world.base().await?;

        let executor_class_hash = provider
            .get_class_hash_at(BLOCK_ID, executor_address)
            .await
            .map_err(|err| match err {
                ProviderError::StarknetError(StarknetErrorWithMessage {
                    code: MaybeUnknownErrorCode::Known(StarknetError::ContractNotFound),
                    ..
                }) => ManifestError::ExecutorNotFound,
                err => err.into(),
            })?;

        let models = get_remote_world_registered_models(world_address, &provider).await.unwrap();
        let contracts = get_remote_world_deployed_contracts(world_address, provider).await.unwrap();

        Ok(Manifest {
            models,
            contracts,
            world: Contract {
                name: WORLD_CONTRACT_NAME.into(),
                class_hash: world_class_hash,
                address: Some(world_address),
                ..Default::default()
            },
            executor: Contract {
                name: EXECUTOR_CONTRACT_NAME.into(),
                address: Some(executor_address),
                class_hash: executor_class_hash,
                ..Default::default()
            },
            base: Class {
                name: BASE_CONTRACT_NAME.into(),
                class_hash: base_class_hash,
                ..Default::default()
            },
        })
    }
}

pub(self) static MODEL_REGISTERED_EVENT_NAME: &str = "ModelRegistered";
pub(self) static CONTRACT_DEPLOYED_EVENT_NAME: &str = "ContractDeployed";

async fn get_remote_world_deployed_contracts<P>(
    world: FieldElement,
    provider: P,
) -> Result<Vec<Contract>, ManifestError<<P as Provider>::Error>>
where
    P::Error: 'static,
    P: Provider + Send + Sync,
{
    let event_key = vec![starknet_keccak(CONTRACT_DEPLOYED_EVENT_NAME.as_bytes())];

    let mut contracts =
        parse_contract_events(&provider, world, vec![event_key], parse_deployed_contracts_events)
            .await?;

    for (address, contract) in &mut contracts {
        let name = match provider
            .call(
                FunctionCall {
                    calldata: vec![],
                    contract_address: *address,
                    entry_point_selector: selector!("name"),
                },
                BlockId::Tag(BlockTag::Latest),
            )
            .await
        {
            Ok(res) => parse_cairo_short_string(&res[0])?.into(),

            Err(ProviderError::StarknetError(StarknetErrorWithMessage {
                code: MaybeUnknownErrorCode::Known(StarknetError::ContractError),
                ..
            })) => SmolStr::from(""),

            Err(err) => return Err(err.into()),
        };

        contract.name = name;
    }

    Ok(contracts.into_values().collect())
}

async fn get_remote_world_registered_models<P: Provider>(
    world: FieldElement,
    provider: P,
) -> Result<Vec<Model>, ManifestError<<P as Provider>::Error>> {
    let event_key = vec![starknet_keccak(MODEL_REGISTERED_EVENT_NAME.as_bytes())];
    parse_contract_events(provider, world, vec![event_key], parse_registered_model_events)
        .await
        .map_err(|e| e.into())
}

async fn parse_contract_events<P: Provider, T>(
    provider: P,
    world: FieldElement,
    keys: Vec<Vec<FieldElement>>,
    f: impl FnOnce(Vec<EmittedEvent>) -> T,
) -> Result<T, ProviderError<<P as Provider>::Error>> {
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

    Ok(f(events))
}

fn parse_deployed_contracts_events(events: Vec<EmittedEvent>) -> HashMap<FieldElement, Contract> {
    events
        .into_iter()
        .map(|event| {
            let mut data = event.data.into_iter();

            let _ = data.next().expect("salt is missing from event");
            let class_hash = data.next().expect("class hash is missing from event");
            let address = data.next().expect("addresss is missing from event");

            (address, Contract { address: Some(address), class_hash, ..Default::default() })
        })
        .collect()
}

fn parse_registered_model_events(events: Vec<EmittedEvent>) -> Vec<Model> {
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
        .map(|(name, class_hash)| Model { name, class_hash, ..Default::default() })
        .collect()
}
