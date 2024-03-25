use anyhow::{Context, Result};
use dojo_world::contracts::world::WorldContract;
use dojo_world::migration::TxConfig;
use starknet::accounts::{Call, ConnectedAccount};
use starknet::core::types::FieldElement;
use starknet::core::utils::get_selector_from_name;

use super::get_contract_address;
use crate::utils::handle_transaction_result;

pub async fn execute<A>(
    contract: String,
    entrypoint: String,
    calldata: Vec<FieldElement>,
    world: WorldContract<A>,
    transaction: TxConfig,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
{
    let contract_address = get_contract_address(&world, contract).await?;
    let res = world
        .account
        .execute(vec![Call {
            calldata,
            to: contract_address,
            selector: get_selector_from_name(&entrypoint)?,
        }])
        .send()
        .await
        .with_context(|| "Failed to send transaction")?;

    handle_transaction_result(&world.account.provider(), res, transaction.wait, transaction.receipt)
        .await
}
