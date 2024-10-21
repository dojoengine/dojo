use anyhow::Result;
use dojo_utils::TxnConfig;
use dojo_world::contracts::world::WorldContract;
use scarb_ui::Ui;
#[cfg(feature = "walnut")]
use sozo_walnut::WalnutDebugger;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{Call, Felt};
use starknet::core::utils::get_selector_from_name;

use crate::utils;

pub async fn execute<A>(
    ui: &Ui,
    tag_or_address: String,
    entrypoint: String,
    calldata: Vec<Felt>,
    world: &WorldContract<A>,
    txn_config: &TxnConfig,
    #[cfg(feature = "walnut")] walnut_debugger: &Option<WalnutDebugger>,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
{
    let contract_address = utils::get_contract_address(world, &tag_or_address).await?;
    let call =
        Call { calldata, to: contract_address, selector: get_selector_from_name(&entrypoint)? };

    let Some(invoke_res) =
        dojo_utils::handle_execute(txn_config.fee_setting, &world.account, vec![call]).await?
    else {
        todo!("handle estimate and simulate")
    };

    utils::handle_transaction_result(
        ui,
        &world.account.provider(),
        invoke_res,
        txn_config.wait,
        txn_config.receipt,
        #[cfg(feature = "walnut")]
        walnut_debugger,
    )
    .await
}
