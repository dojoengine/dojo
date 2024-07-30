use std::time::Duration;

use super::STARKNET_ACCOUNT;
use anyhow::{bail, Context};
use dojo_world::migration::TxnConfig;
use dojo_world::utils::TransactionExt;
use starknet::accounts::{Account, Call, ConnectedAccount};
use starknet::core::types::{FieldElement, TransactionExecutionStatus, TransactionStatus};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;
use tokio::time::sleep;

pub struct PiltoverArgs {
    pub contract: FieldElement,
    pub nonce: FieldElement,
}

pub async fn starknet_apply_piltover(
    piltover_contract: FieldElement,
    nonce: FieldElement,
) -> anyhow::Result<String> {
    let txn_config = TxnConfig { wait: true, receipt: true, ..Default::default() };
    let tx = STARKNET_ACCOUNT
        .execute(vec![Call {
            to: piltover_contract,
            selector: get_selector_from_name("update_state").expect("invalid selector"),
            calldata: todo!(),
        }])
        .nonce(nonce)
        .send_with_cfg(&txn_config)
        .await
        .context("Failed to send `update_state` transaction.")?;

    let start_fetching = std::time::Instant::now();
    let wait_for = Duration::from_secs(60);
    let execution_status = loop {
        if start_fetching.elapsed() > wait_for {
            bail!("Transaction not mined in {} seconds.", wait_for.as_secs());
        }

        let status =
            match STARKNET_ACCOUNT.provider().get_transaction_status(tx.transaction_hash).await {
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
                bail!("Transaction {:#x} rejected.", tx.transaction_hash);
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
            bail!("Transaction failed with.");
        }
    }

    Ok(format!("{:#x}", tx.transaction_hash))
}
