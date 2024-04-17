use std::time::Duration;

use crate::starknet_os::STARKNET_ACCOUNT;
use starknet::accounts::{Account, Call, ConnectedAccount};
use starknet::core::types::{FieldElement, TransactionExecutionStatus, TransactionStatus};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;
use tokio::time::sleep;

pub async fn starknet_verify(
    fact_registry_address: FieldElement,
    serialized_proof: Vec<FieldElement>,
) -> anyhow::Result<String> {
    let tx = STARKNET_ACCOUNT
        .execute(vec![Call {
            to: fact_registry_address,
            selector: get_selector_from_name("verify_and_register_fact").expect("invalid selector"),
            calldata: serialized_proof,
        }])
        .send()
        .await?;

    let start_fetching = std::time::Instant::now();
    let wait_for = Duration::from_secs(60);
    let execution_status = loop {
        if start_fetching.elapsed() > wait_for {
            anyhow::bail!("Transaction not mined in {} seconds.", wait_for.as_secs());
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

    Ok(format!("{:#x}", tx.transaction_hash))
}
