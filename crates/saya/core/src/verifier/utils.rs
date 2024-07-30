use std::time::Duration;

use starknet::{
    accounts::ConnectedAccount,
    core::types::{InvokeTransactionResult, TransactionExecutionStatus, TransactionStatus},
    providers::Provider,
};
use tokio::time::sleep;
use tracing::trace;

use crate::dojo_os::STARKNET_ACCOUNT;

pub async fn wait_for_sent_transaction(tx: InvokeTransactionResult) -> anyhow::Result<()> {
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
                trace!("Transaction received.");
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
            trace!("Transaction accepted on L2.");
        }
        TransactionExecutionStatus::Reverted => {
            anyhow::bail!("Transaction failed with.");
        }
    }

    sleep(Duration::from_secs(3)).await; // Sometimes fails in

    Ok(())
}
