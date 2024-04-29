use anyhow::{Context, Result};
use dojo_world::contracts::world::WorldContract;
use dojo_world::migration::TxnConfig;
use dojo_world::utils::TransactionExt;
use starknet::accounts::{Call, ConnectedAccount};
use starknet::core::types::FieldElement;
use starknet::core::utils::get_selector_from_name;

use crate::utils;

pub async fn execute<A>(
    contract: String,
    entrypoint: String,
    calldata: Vec<FieldElement>,
    world: &WorldContract<A>,
    txn_config: &TxnConfig,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
{
    let contract_address = utils::get_contract_address(world, contract).await?;
    let res = world
        .account
        .execute(vec![Call {
            calldata,
            to: contract_address,
            selector: get_selector_from_name(&entrypoint)?,
        }])
        .send_with_cfg(txn_config)
        .await
        .with_context(|| "Failed to send transaction")?;

    utils::handle_transaction_result(
        &world.account.provider(),
        res,
        txn_config.wait,
        txn_config.receipt,
    )
    .await
}
