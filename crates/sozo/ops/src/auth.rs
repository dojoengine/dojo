use std::str::FromStr;

use anyhow::{Context, Result};
use dojo_utils::{TransactionExt, TransactionWaiter, TxnConfig};
use dojo_world::contracts::model::ModelError;
use dojo_world::contracts::naming::{
    compute_bytearray_hash, compute_selector_from_tag, ensure_namespace,
};
use dojo_world::contracts::world::WorldContract;
use dojo_world::contracts::WorldContractReader;
use scarb_ui::Ui;
use sozo_walnut::WalnutDebugger;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::{BlockId, BlockTag, Felt};

use crate::migration::ui::MigrationUi;
use crate::utils;

#[derive(Debug, Clone, PartialEq)]
pub enum ResourceType {
    Contract(String),
    Namespace(String),
    Model(String),
    // this can be a selector for any other resource type
    Selector(Felt),
}

impl FromStr for ResourceType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split_once(':');
        let resource = match parts {
            Some(("contract", name)) | Some(("c", name)) => {
                ResourceType::Contract(name.to_string())
            }
            Some(("model", name)) | Some(("m", name)) => ResourceType::Model(name.to_string()),
            Some(("namespace", name)) | Some(("ns", name)) => {
                ResourceType::Namespace(name.to_string())
            }
            Some(("selector", name)) | Some(("s", name)) => {
                ResourceType::Selector(Felt::from_str(name)?)
            }
            _ => anyhow::bail!(format!(
                "Resource is expected to be in the format `resource_type:resource_name`: `sozo \
                 auth grant owner resource_type:resource_name,0x1234`, Found: {}.",
                s
            )),
        };
        Ok(resource)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceWriter {
    pub resource: ResourceType,
    pub tag_or_address: String,
}

impl FromStr for ResourceWriter {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(',').collect();

        let (resource, tag_or_address) = match parts.as_slice() {
            [resource, tag_or_address] => (resource, tag_or_address.to_string()),
            _ => anyhow::bail!(
                "Resource and contract are expected to be comma separated: `sozo auth grant \
                 writer model:model_name,0x1234`"
            ),
        };

        let resource = ResourceType::from_str(resource)?;
        Ok(ResourceWriter { resource, tag_or_address })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceOwner {
    pub resource: ResourceType,
    pub owner: Felt,
}

impl FromStr for ResourceOwner {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(',').collect();

        let (resource, owner) = match parts.as_slice() {
            [resource, owner] => (resource, owner),
            _ => anyhow::bail!(
                "Resource and owner are expected to be comma separated: `sozo auth grant owner \
                 resource_type:resource_name,0x1234`"
            ),
        };

        let owner = Felt::from_hex(owner)
            .map_err(|_| anyhow::anyhow!("Invalid owner address: {}", owner))?;

        let resource = ResourceType::from_str(resource)?;

        Ok(ResourceOwner { owner, resource })
    }
}

pub async fn grant_writer<'a, A>(
    ui: &'a Ui,
    world: &WorldContract<A>,
    new_writers: &[ResourceWriter],
    txn_config: &TxnConfig,
    default_namespace: &str,
    walnut_debugger: &Option<WalnutDebugger>,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send,
    <A as Account>::SignError: 'static,
{
    let mut calls = Vec::new();

    for new_writer in new_writers {
        let resource_selector =
            get_resource_selector(ui, world, &new_writer.resource, default_namespace).await?;
        let contract_address =
            utils::get_contract_address(world, &new_writer.tag_or_address).await?;
        calls.push(world.grant_writer_getcall(&resource_selector, &contract_address.into()));
    }

    if !calls.is_empty() {
        let res = world
            .account
            .execute_v1(calls)
            .send_with_cfg(txn_config)
            .await
            .with_context(|| "Failed to send transaction")?;

        TransactionWaiter::new(res.transaction_hash, &world.provider()).await?;

        utils::handle_transaction_result(
            ui,
            &world.account.provider(),
            res,
            txn_config.wait,
            txn_config.receipt,
            walnut_debugger,
        )
        .await?;
    }

    Ok(())
}

pub async fn grant_owner<A>(
    ui: &Ui,
    world: &WorldContract<A>,
    new_owners: &[ResourceOwner],
    txn_config: &TxnConfig,
    default_namespace: &str,
    walnut_debugger: &Option<WalnutDebugger>,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
{
    let mut calls = Vec::new();

    for new_owner in new_owners {
        let resource_selector =
            get_resource_selector(ui, world, &new_owner.resource, default_namespace).await?;
        calls.push(world.grant_owner_getcall(&resource_selector, &new_owner.owner.into()));
    }

    let res = world
        .account
        .execute_v1(calls)
        .send_with_cfg(txn_config)
        .await
        .with_context(|| "Failed to send transaction")?;

    TransactionWaiter::new(res.transaction_hash, &world.provider()).await?;

    utils::handle_transaction_result(
        ui,
        &world.account.provider(),
        res,
        txn_config.wait,
        txn_config.receipt,
        walnut_debugger,
    )
    .await?;

    Ok(())
}

pub async fn revoke_writer<A>(
    ui: &Ui,
    world: &WorldContract<A>,
    new_writers: &[ResourceWriter],
    txn_config: &TxnConfig,
    default_namespace: &str,
    walnut_debugger: &Option<WalnutDebugger>,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
{
    let mut calls = Vec::new();

    for new_writer in new_writers {
        let resource_selector =
            get_resource_selector(ui, world, &new_writer.resource, default_namespace).await?;
        let contract_address =
            utils::get_contract_address(world, &new_writer.tag_or_address).await?;
        calls.push(world.revoke_writer_getcall(&resource_selector, &contract_address.into()));
    }

    if !calls.is_empty() {
        let res = world
            .account
            .execute_v1(calls)
            .send_with_cfg(txn_config)
            .await
            .with_context(|| "Failed to send transaction")?;

        TransactionWaiter::new(res.transaction_hash, &world.provider()).await?;

        utils::handle_transaction_result(
            ui,
            &world.account.provider(),
            res,
            txn_config.wait,
            txn_config.receipt,
            walnut_debugger,
        )
        .await?;
    }

    Ok(())
}

pub async fn revoke_owner<A>(
    ui: &Ui,
    world: &WorldContract<A>,
    new_owners: &[ResourceOwner],
    txn_config: &TxnConfig,
    default_namespace: &str,
    walnut_debugger: &Option<WalnutDebugger>,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
{
    let mut calls = Vec::new();

    for new_owner in new_owners {
        let resource_selector =
            get_resource_selector(ui, world, &new_owner.resource, default_namespace).await?;
        calls.push(world.revoke_owner_getcall(&resource_selector, &new_owner.owner.into()));
    }

    let res = world
        .account
        .execute_v1(calls)
        .send_with_cfg(txn_config)
        .await
        .with_context(|| "Failed to send transaction")?;

    utils::handle_transaction_result(
        ui,
        &world.account.provider(),
        res,
        txn_config.wait,
        txn_config.receipt,
        walnut_debugger,
    )
    .await?;

    Ok(())
}

pub async fn get_resource_selector<A>(
    ui: &Ui,
    world: &WorldContract<A>,
    resource: &ResourceType,
    default_namespace: &str,
) -> Result<Felt>
where
    A: ConnectedAccount + Sync + Send,
    <A as Account>::SignError: 'static,
{
    let world_reader = WorldContractReader::new(world.address, world.account.provider())
        .with_block(BlockId::Tag(BlockTag::Pending));

    let resource_selector = match resource {
        ResourceType::Contract(tag_or_address) => {
            let tag_or_address = if tag_or_address.starts_with("0x") {
                tag_or_address.to_string()
            } else {
                ensure_namespace(tag_or_address, default_namespace)
            };
            utils::get_contract_address(world, &tag_or_address).await?
        }
        ResourceType::Model(tag_or_name) => {
            // TODO: Is some models have version 0 (using the name of the struct instead of the
            // selector), we're not able to distinguish that.
            // Should we add the version into the `ModelContract` struct? Can we always know that?
            let tag = ensure_namespace(tag_or_name, default_namespace);

            // be sure that the model exists
            match world_reader.model_reader_with_tag(&tag).await {
                Err(ModelError::ModelNotFound) => {
                    ui.print_sub(format!("Unknown model '{}' => IGNORED", tag));
                }
                Err(err) => {
                    return Err(err.into());
                }
                _ => {}
            };

            compute_selector_from_tag(&tag)
        }
        ResourceType::Namespace(name) => compute_bytearray_hash(name),
        ResourceType::Selector(selector) => *selector,
    };

    Ok(resource_selector)
}
