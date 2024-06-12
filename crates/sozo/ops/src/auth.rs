use std::str::FromStr;

use anyhow::{Context, Result};
use dojo_world::contracts::model::ModelError;
use dojo_world::contracts::world::WorldContract;
use dojo_world::contracts::{cairo_utils, WorldContractReader};
use dojo_world::migration::TxnConfig;
use dojo_world::utils::TransactionExt;
use scarb_ui::Ui;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{BlockId, BlockTag};
use starknet::core::utils::{get_selector_from_name, parse_cairo_short_string};
use starknet_crypto::FieldElement;

use crate::utils;

#[derive(Debug, Clone, PartialEq)]
pub enum ResourceType {
    Contract(String),
    Model(FieldElement),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelContract {
    pub model: FieldElement,
    pub contract: String,
}

impl FromStr for ModelContract {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(',').collect();

        let (model, contract) = match parts.as_slice() {
            [model, contract] => (model, contract),
            _ => anyhow::bail!(
                "Model and contract address are expected to be comma separated: `sozo auth grant \
                 writer model_name,0x1234`"
            ),
        };

        let model = cairo_utils::str_to_felt(model)
            .map_err(|_| anyhow::anyhow!("Invalid model name: {}", model))?;

        Ok(ModelContract { model, contract: contract.to_string() })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OwnerResource {
    pub resource: ResourceType,
    pub owner: FieldElement,
}

impl FromStr for OwnerResource {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(',').collect();

        let (resource_part, owner_part) = match parts.as_slice() {
            [resource, owner] => (*resource, *owner),
            _ => anyhow::bail!(
                "Owner and resource are expected to be comma separated: `sozo auth grant owner \
                 resource_type:resource_name,0x1234`"
            ),
        };

        let owner = FieldElement::from_hex_be(owner_part)
            .map_err(|_| anyhow::anyhow!("Invalid owner address: {}", owner_part))?;

        let resource_parts = resource_part.split_once(':');
        let resource = match resource_parts {
            Some(("contract", name)) => ResourceType::Contract(name.to_string()),
            Some(("model", name)) => {
                let model = cairo_utils::str_to_felt(name)
                    .map_err(|_| anyhow::anyhow!("Invalid model name: {}", name))?;
                ResourceType::Model(model)
            }
            _ => anyhow::bail!(
                "Resource is expected to be in the format `resource_type:resource_name`: `sozo \
                 auth grant owner resource_type:resource_name,0x1234`"
            ),
        };

        Ok(OwnerResource { owner, resource })
    }
}

pub async fn grant_writer<A>(
    ui: &Ui,
    world: &WorldContract<A>,
    models_contracts: Vec<ModelContract>,
    txn_config: TxnConfig,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
{
    let mut calls = Vec::new();

    let world_reader = WorldContractReader::new(world.address, world.account.provider())
        .with_block(BlockId::Tag(BlockTag::Pending));

    // TODO: Is some models have version 0 (using the name of the struct instead of the selector),
    // we're not able to distinguish that.
    // Should we add the version into the `ModelContract` struct? Can we always know that?
    for mc in models_contracts {
        let model_name = parse_cairo_short_string(&mc.model)?;
        let model_selector = get_selector_from_name(&model_name)?;

        match world_reader.model_reader(&model_name).await {
            Ok(_) => {
                let contract = utils::get_contract_address(world, mc.contract).await?;
                calls.push(world.grant_writer_getcall(&model_selector, &contract.into()));
            }

            Err(ModelError::ModelNotFound) => {
                ui.print(format!("Unknown model '{}' => IGNORED", model_name));
            }

            Err(err) => {
                return Err(err.into());
            }
        }
    }

    if !calls.is_empty() {
        let res = world
            .account
            .execute_v1(calls)
            .send_with_cfg(&txn_config)
            .await
            .with_context(|| "Failed to send transaction")?;

        utils::handle_transaction_result(
            ui,
            &world.account.provider(),
            res,
            txn_config.wait,
            txn_config.receipt,
        )
        .await?;
    }

    Ok(())
}

pub async fn grant_owner<A>(
    ui: &Ui,
    world: &WorldContract<A>,
    owners_resources: Vec<OwnerResource>,
    txn_config: TxnConfig,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
{
    let mut calls = Vec::new();

    for or in owners_resources {
        let resource = match &or.resource {
            ResourceType::Model(name) => {
                // TODO: Is some models have version 0 (using the name of the struct instead of the
                // selector), we're not able to distinguish that.
                // Should we add the version into the `ModelContract` struct? Can we always know
                // that?
                let model_name = parse_cairo_short_string(name)?;
                get_selector_from_name(&model_name)?
            }
            ResourceType::Contract(name_or_address) => {
                utils::get_contract_address(world, name_or_address.clone()).await?
            }
        };

        calls.push(world.grant_owner_getcall(&or.owner.into(), &resource));
    }

    let res = world
        .account
        .execute_v1(calls)
        .send_with_cfg(&txn_config)
        .await
        .with_context(|| "Failed to send transaction")?;

    utils::handle_transaction_result(
        ui,
        &world.account.provider(),
        res,
        txn_config.wait,
        txn_config.receipt,
    )
    .await?;

    Ok(())
}

pub async fn revoke_writer<A>(
    ui: &Ui,
    world: &WorldContract<A>,
    models_contracts: Vec<ModelContract>,
    txn_config: TxnConfig,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
{
    let mut calls = Vec::new();

    let mut world_reader = WorldContractReader::new(world.address, world.account.provider());
    world_reader.set_block(BlockId::Tag(BlockTag::Pending));

    for mc in models_contracts {
        // TODO: Is some models have version 0 (using the name of the struct instead of the
        // selector), we're not able to distinguish that.
        // Should we add the version into the `ModelContract` struct? Can we always know that?
        let model_name = parse_cairo_short_string(&mc.model)?;
        let model_selector = get_selector_from_name(&model_name)?;

        match world_reader.model_reader(&model_name).await {
            Ok(_) => {
                let contract = utils::get_contract_address(world, mc.contract).await?;
                calls.push(world.revoke_writer_getcall(&model_selector, &contract.into()));
            }

            Err(ModelError::ModelNotFound) => {
                ui.print(format!("Unknown model '{}' => IGNORED", model_name));
            }

            Err(err) => {
                return Err(err.into());
            }
        }
    }

    if !calls.is_empty() {
        let res = world
            .account
            .execute_v1(calls)
            .send_with_cfg(&txn_config)
            .await
            .with_context(|| "Failed to send transaction")?;

        utils::handle_transaction_result(
            ui,
            &world.account.provider(),
            res,
            txn_config.wait,
            txn_config.receipt,
        )
        .await?;
    }

    Ok(())
}

pub async fn revoke_owner<A>(
    ui: &Ui,
    world: &WorldContract<A>,
    owners_resources: Vec<OwnerResource>,
    txn_config: TxnConfig,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
{
    let mut calls = Vec::new();

    for or in owners_resources {
        let resource = match &or.resource {
            ResourceType::Model(name) => {
                // TODO: Is some models have version 0 (using the name of the struct instead of the
                // selector), we're not able to distinguish that.
                // Should we add the version into the `ModelContract` struct? Can we always know
                // that?
                let model_name = parse_cairo_short_string(name)?;
                get_selector_from_name(&model_name)?
            }
            ResourceType::Contract(name_or_address) => {
                utils::get_contract_address(world, name_or_address.clone()).await?
            }
        };

        calls.push(world.revoke_owner_getcall(&or.owner.into(), &resource));
    }

    let res = world
        .account
        .execute_v1(calls)
        .send_with_cfg(&txn_config)
        .await
        .with_context(|| "Failed to send transaction")?;

    utils::handle_transaction_result(
        ui,
        &world.account.provider(),
        res,
        txn_config.wait,
        txn_config.receipt,
    )
    .await?;

    Ok(())
}
