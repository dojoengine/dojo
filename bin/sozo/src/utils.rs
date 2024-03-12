use anyhow::Result;
use dojo_world::utils::{execution_status_from_maybe_pending_receipt, TransactionWaiter};
use starknet::core::types::{ExecutionResult, InvokeTransactionResult};
use starknet::providers::Provider;

pub async fn handle_transaction_result<P>(
    provider: P,
    transaction_result: InvokeTransactionResult,
    wait_for_tx: bool,
    show_receipt: bool,
) -> Result<()>
where
    P: Provider + Send,
{
    println!("\nTransaction hash: {:#x}", transaction_result.transaction_hash);

    if wait_for_tx {
        let receipt =
            TransactionWaiter::new(transaction_result.transaction_hash, &provider).await?;

        if show_receipt {
            println!("Receipt:\n{}", serde_json::to_string_pretty(&receipt)?);
        } else {
            match execution_status_from_maybe_pending_receipt(&receipt) {
                ExecutionResult::Succeeded => {
                    println!("Status: OK");
                }
                ExecutionResult::Reverted { reason } => {
                    println!("Status: REVERTED");
                    println!("Reason:\n{}", reason);
                }
            };
        }
    }

    Ok(())
}
