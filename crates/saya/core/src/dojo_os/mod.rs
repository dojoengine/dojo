//! Starknet OS types.
// SNOS is based on blockifier, which is not in sync with
// current primitives.
// And SNOS is for now not used. This work must be resume once
// SNOS is actualized.
// mod felt;
// pub mod input;
// pub mod transaction;

pub mod piltover;

use std::time::Duration;

use dojo_utils::{TransactionExt, TxnConfig};
use itertools::chain;
use starknet::accounts::{Account,ConnectedAccount};
use starknet::core::types::Call;
use starknet::core::types::{Felt, TransactionExecutionStatus, TransactionStatus};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;
use tokio::time::sleep;
use tracing::trace;

use crate::{error::Error, SayaStarknetAccount};
use crate::{retry, LOG_TARGET};

pub async fn starknet_apply_diffs(
    world: Felt,
    new_state: Vec<Felt>,
    program_output: Vec<Felt>,
    program_hash: Felt,
    account: &SayaStarknetAccount,
    nonce: Felt,
) -> Result<String, Error> {
    let calldata: Vec<Felt> = chain![
        [Felt::from(new_state.len() as u64 / 2)].into_iter(),
        new_state.clone().into_iter(),
        program_output.into_iter(),
        [program_hash],
    ]
    .collect();

    let txn_config = TxnConfig { wait: true, receipt: true, ..Default::default() };
    let tx = retry!(account
        .execute_v1(vec![Call {
            to: world,
            selector: get_selector_from_name("upgrade_state").expect("invalid selector"),
            calldata: calldata.clone(),
        }])
        .nonce(nonce)
        .send_with_cfg(&txn_config))
    .map_err(|e| Error::TransactionFailed(e.to_string()))?;

    let start_fetching = std::time::Instant::now();
    let wait_for = Duration::from_secs(60);
    let execution_status = loop {
        if start_fetching.elapsed() > wait_for {
            return Err(Error::TimeoutError(format!(
                "Transaction not mined in {} seconds.",
                wait_for.as_secs()
            )));
        }

        let status = match account.provider().get_transaction_status(tx.transaction_hash).await {
            Ok(status) => status,
            Err(_e) => {
                sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        break match status {
            TransactionStatus::Received => {
                println!("Transaction received.");
                sleep(Duration::from_secs(1)).await;
                continue;
            }
            TransactionStatus::Rejected => {
                return Err(Error::TransactionRejected(tx.transaction_hash.to_string()));
            }
            TransactionStatus::AcceptedOnL2(execution_status) => execution_status,
            TransactionStatus::AcceptedOnL1(execution_status) => execution_status,
        };
    };

    match execution_status {
        TransactionExecutionStatus::Succeeded => {
            trace!(target: LOG_TARGET, "Transaction accepted on L2.");
        }
        TransactionExecutionStatus::Reverted => {
            return Err(Error::TransactionFailed(tx.transaction_hash.to_string()));
        }
    }

    Ok(format!("{:#x}", tx.transaction_hash))
}
