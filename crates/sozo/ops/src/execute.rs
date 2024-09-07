use anyhow::{Context, Result};
use dojo_utils::{TransactionExt, TxnConfig};
use dojo_world::contracts::world::WorldContract;
use scarb_ui::Ui;
use sozo_walnut::WalnutDebugger;
use starknet::accounts::{Call, ConnectedAccount};
use starknet::core::types::Felt;
use starknet::core::utils::get_selector_from_name;

use crate::utils;

pub async fn execute<A>(
    ui: &Ui,
    tag_or_address: String,
    entrypoint: String,
    calldata: Vec<Felt>,
    world: &WorldContract<A>,
    txn_config: &TxnConfig,
    walnut_debugger: &Option<WalnutDebugger>,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
{
    let contract_address = utils::get_contract_address(world, &tag_or_address).await?;
    let res = world
        .account
        .execute_v1(vec![Call {
            calldata,
            to: contract_address,
            selector: get_selector_from_name(&entrypoint)?,
        }])
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
    .await
}
