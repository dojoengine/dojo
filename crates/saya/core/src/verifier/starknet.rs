use std::time::Duration;

use anyhow::Context;
use dojo_utils::{TransactionExt, TxnConfig};
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::{Call, Felt, TransactionExecutionStatus, TransactionStatus};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;
use tokio::time::sleep;

use crate::dojo_os::get_starknet_account;
use crate::StarknetAccountData;

pub async fn starknet_verify(
    fact_registry_address: Felt,
    serialized_proof: Vec<Felt>,
    starknet_config: StarknetAccountData,
) -> anyhow::Result<(String, Felt)> {
    let txn_config = TxnConfig { wait: true, receipt: true, ..Default::default() };
    let account = get_starknet_account(starknet_config)?;
    let account = account.lock().await;

    let nonce = account.get_nonce().await?;
    let tx = account
        .execute_v1(vec![Call {
            to: fact_registry_address,
            selector: get_selector_from_name("verify_and_register_fact").expect("invalid selector"),
            calldata: serialized_proof,
        }])
        .nonce(nonce)
        .send_with_cfg(&txn_config)
        .await
        .context("Failed to send `verify_and_register_fact` transaction.")?;

    let start_fetching = std::time::Instant::now();
    let wait_for = Duration::from_secs(60);
    let execution_status = loop {
        if start_fetching.elapsed() > wait_for {
            anyhow::bail!("Transaction not mined in {} seconds.", wait_for.as_secs());
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
                anyhow::bail!("Transaction {:#x} rejected.", tx.transaction_hash);
            }
            TransactionStatus::AcceptedOnL2(execution_status) => execution_status,
            TransactionStatus::AcceptedOnL1(execution_status) => execution_status,
        };
    };

    match execution_status {
        TransactionExecutionStatus::Succeeded => {
            println!("Transaction accepted on L2.");
        }
        TransactionExecutionStatus::Reverted => {
            anyhow::bail!("Transaction failed with.");
        }
    }

    Ok((format!("{:#x}", tx.transaction_hash), nonce))
}
