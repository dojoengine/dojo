use starknet::accounts::ConnectedAccount;
use starknet::core::types::{
    InvokeTransactionResult, TransactionExecutionStatus, TransactionStatus,
};
use starknet::providers::Provider;
use std::time::Duration;
use tokio::time::sleep;
use tracing::trace;

use crate::error::Error;
use crate::{SayaStarknetAccount, LOG_TARGET};

pub async fn wait_for_sent_transaction(
    tx: InvokeTransactionResult,
    account: &SayaStarknetAccount,
) -> Result<(), Error> {
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
                trace!(target: LOG_TARGET, "Transaction received.");
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
            // Return a custom error when the transaction is reverted
            return Err(Error::TransactionFailed(tx.transaction_hash.to_string()));
        }
    }

    Ok(())
}
