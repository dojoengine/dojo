use anyhow::Error;
use async_trait::async_trait;
use smol_str::SmolStr;
use starknet::core::types::{
    BlockId, BlockTag, EmittedEvent, EventFilter, FieldElement, FunctionCall, StarknetError,
};
use starknet::core::utils::{parse_cairo_short_string, starknet_keccak};
use starknet::macros::selector;
use starknet::providers::{Provider, ProviderError};
use std::collections::HashMap;

use crate::contracts::WorldContractReader;
use crate::manifests::manifest::{
    Contract, World, BASE_CONTRACT_NAME, EXECUTOR_CONTRACT_NAME, WORLD_CONTRACT_NAME,
};

use super::manifest::{Manifest, ManifestKind, WorldError};

#[async_trait]
pub trait RemoteLoadable<P: Provider + Sync + Send> {
    async fn load_from_remote(
        provider: P,
        world_address: FieldElement,
    ) -> Result<World, WorldError>;
}

#[async_trait]
impl<P: Provider + Sync + Send> RemoteLoadable<P> for World {
    /// Construct a manifest of a remote World.
    ///
    /// # Arguments
    /// * `provider` - A Starknet RPC provider.
    /// * `world_address` - The address of the remote World contract.
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
            remote_models_and_contracts(world_address, &world.provider()).await?;

        // Err(WorldError::RemoteWorldNotFound)
        Ok(World {
            models,
            contracts,
            world: Manifest {
                name: WORLD_CONTRACT_NAME.into(),
                class_hash: world_class_hash,
                kind: ManifestKind::Contract(Contract { address: world_address }),
            },
            executor: Manifest {
                name: EXECUTOR_CONTRACT_NAME.into(),
                class_hash: executor_class_hash,
                kind: ManifestKind::Contract(Contract { address: executor_address.into() }),
            },
            base: Manifest {
                name: BASE_CONTRACT_NAME.into(),
                class_hash: base_class_hash.into(),
                kind: ManifestKind::Class,
            },
        })
    }
}

async fn remote_models_and_contracts<P: Provider>(
    world: FieldElement,
    provider: P,
) -> Result<(Vec<Manifest>, Vec<Manifest>), WorldError>
where
    P: Provider + Send + Sync,
{
    let registered_models_event_name = starknet_keccak("ModelRegistered".as_bytes());
    let contract_deployed_event_name = starknet_keccak("ContractDeployed".as_bytes());
    let contract_upgraded_event_name = starknet_keccak("ContractUpgraded".as_bytes());

    let events = events(
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
                    contract_address: contract.address.expect("qed; missing address"),
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

async fn events<P: Provider>(
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

            Manifest { kind: ManifestKind::Contract(Contract { address }), class_hash }
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
            kind: ManifestKind::Contract,
            name: name.into(),
            class_hash,
        })
        .collect()
}
