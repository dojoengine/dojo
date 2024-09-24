use std::time::Duration;

use cairo_proof_parser::to_felts;
use dojo_utils::{TransactionExt, TxnConfig};
use serde::Serialize;
use starknet::accounts::{Account, Call, ConnectedAccount};
use starknet::core::utils::get_selector_from_name;
use starknet_crypto::Felt;
use tokio::time::sleep;
use tracing::trace;

use crate::verifier::utils::wait_for_sent_transaction;
use crate::{retry, SayaStarknetAccount, LOG_TARGET};

#[derive(Debug, Serialize)]
pub struct PiltoverCalldata {
    pub program_output: Vec<Felt>,
    pub onchain_data_hash: Felt,
    pub onchain_data_size: (Felt, Felt), // U256
}

pub async fn starknet_apply_piltover(
    calldata: PiltoverCalldata,
    contract: Felt,
    account: &SayaStarknetAccount,
) -> anyhow::Result<()> {
    sleep(Duration::from_secs(2)).await;
    let nonce = account.get_nonce().await?;
    let txn_config = TxnConfig { wait: true, receipt: true, ..Default::default() };
    let calldata = to_felts(&calldata)?;
    trace!(target: LOG_TARGET, "Sending `update_state` piltover transaction to contract {:#x}", contract);
    let tx = retry!(account
        .execute_v1(vec![Call {
            to: contract,
            selector: get_selector_from_name("update_state").expect("invalid selector"),
            calldata: calldata.clone()
        }])
        .nonce(nonce)
        .send_with_cfg(&txn_config))
    .unwrap();
    trace!(target: LOG_TARGET,  "Sent `update_state` piltover transaction {:#x}", tx.transaction_hash);
    wait_for_sent_transaction(tx, account).await?;

    Ok(())
}
