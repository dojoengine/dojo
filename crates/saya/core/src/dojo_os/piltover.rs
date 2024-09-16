use cairo_proof_parser::to_felts;
use dojo_utils::{TransactionExt, TxnConfig};
use serde::Serialize;
use starknet::accounts::{Account, Call, ConnectedAccount};
use starknet::core::utils::get_selector_from_name;
use starknet_crypto::Felt;
use tracing::trace;

use crate::verifier::utils::wait_for_sent_transaction;
use crate::{SayaStarknetAccount, LOG_TARGET};

const MAX_TRIES: usize = 30;

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
    mut nonce: Felt,
) -> anyhow::Result<()> {
    let txn_config = TxnConfig { wait: true, receipt: true,..Default::default() };
    let calldata = to_felts(&calldata)?;
    trace!(target: LOG_TARGET, "Sending `update_state` piltover transaction to contract {:#x}", contract);
    let mut tries = 0;
    let tx = loop{
        let tx = account
        .execute_v1(vec![Call {
            to: contract,
            selector: get_selector_from_name("update_state").expect("invalid selector"),
            calldata: calldata.clone()
        }])
        .nonce(nonce)
        .send_with_cfg(&txn_config)
        .await;
        if let Err(e) = tx {
            dbg!(e);
            if tries >= MAX_TRIES {
                anyhow::bail!("Failed to send `update_state` piltover transaction after {} tries.", MAX_TRIES);
            }
            tries += 1;
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            nonce = account.get_nonce().await?;
            continue;
        }else {
            break tx?; 
        }
    };
    trace!(target: LOG_TARGET,  "Sent `update_state` piltover transaction {:#x}", tx.transaction_hash);

    wait_for_sent_transaction(tx, account).await?;

    Ok(())
}
