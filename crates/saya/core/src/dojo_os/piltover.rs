use crate::verifier::utils::wait_for_sent_transaction;

use super::STARKNET_ACCOUNT;
use anyhow::Context;
use cairo_proof_parser::to_felts;
use dojo_world::migration::TxnConfig;
use dojo_world::utils::TransactionExt;
use serde::Serialize;
use starknet::accounts::{Account, Call};
use starknet::core::types::FieldElement;
use starknet::core::utils::get_selector_from_name;
use tracing::trace;

#[derive(Serialize)]
pub struct PiltoverCalldata {
    pub program_output: Vec<FieldElement>,
    pub onchain_data_hash: FieldElement,
    pub onchain_data_size: (FieldElement, FieldElement),
}

pub async fn starknet_apply_piltover(
    calldata: PiltoverCalldata,
    contract: FieldElement,
    nonce: FieldElement,
) -> anyhow::Result<()> {
    let txn_config = TxnConfig { wait: true, receipt: true, ..Default::default() };

    let calldata = to_felts(&calldata)?;

    let tx = STARKNET_ACCOUNT
        .execute(vec![Call {
            to: contract,
            selector: get_selector_from_name("update_state").expect("invalid selector"),
            calldata,
        }])
        .nonce(nonce)
        .send_with_cfg(&txn_config)
        .await
        .context("Failed to send `update_state` transaction.")?;

    trace!("Sent `update_state` piltover transaction {:#x}", tx.transaction_hash);

    wait_for_sent_transaction(tx).await?;

    Ok(())
}
