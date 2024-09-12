use anyhow::Context;
use cairo_proof_parser::to_felts;
use dojo_utils::{TransactionExt, TxnConfig};
use katana_rpc_types::trace;
use serde::Serialize;
use starknet::accounts::{Account, Call};
use starknet::core::utils::get_selector_from_name;
use starknet_crypto::Felt;
use tracing::trace;

use crate::verifier::utils::wait_for_sent_transaction;
use crate::{SayaStarknetAccount, LOG_TARGET};

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
    nonce: Felt,
) -> anyhow::Result<()> {
    let txn_config = TxnConfig { wait: true, receipt: true, ..Default::default() };

    let calldata = to_felts(&calldata)?;
    trace!(target: LOG_TARGET, "Sending `update_state` piltover transaction to contract {:#x}", contract);
    let tx = account
        .execute_v1(vec![Call {
            to: contract,
            selector: get_selector_from_name("update_state").expect("invalid selector"),
            calldata,
        }])
        .nonce(nonce)
        .send_with_cfg(&txn_config)
        .await.unwrap();    
    trace!(target: LOG_TARGET,  "Sent `update_state` piltover transaction {:#x}", tx.transaction_hash);

    wait_for_sent_transaction(tx, account).await?;

    Ok(())
}
