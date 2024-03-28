use anyhow::Result;
use dojo_world::contracts::world::WorldContract;
use dojo_world::migration::strategy::generate_salt;
use dojo_world::utils::{execution_status_from_maybe_pending_receipt, TransactionWaiter};
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{ExecutionResult, FieldElement, InvokeTransactionResult};
use starknet::providers::Provider;

/// Retrieves a contract address from it's name
/// using the world's data, or parses a hex string into
/// a [`FieldElement`].
///
/// # Arguments
///
/// * `world` - The world's contract connector.
/// * `name_or_address` - A string with a contract name or a hexadecimal address.
///
/// # Returns
///
/// A [`FieldElement`] with the address of the contract on success.
pub async fn get_contract_address<A: ConnectedAccount + Sync>(
    world: &WorldContract<A>,
    name_or_address: String,
) -> Result<FieldElement> {
    if name_or_address.starts_with("0x") {
        FieldElement::from_hex_be(&name_or_address).map_err(anyhow::Error::from)
    } else {
        let contract_class_hash = world.base().call().await?;
        Ok(starknet::core::utils::get_contract_address(
            generate_salt(&name_or_address),
            contract_class_hash.into(),
            &[],
            world.address,
        ))
    }
}

/// Handles a transaction result configuring a
/// [`TransactionWaiter`] if required.
///
/// # Arguments
///
/// * `provider` - Starknet provider to fetch transaction status.
/// * `transaction_result` - Result of the transaction to handle.
/// * `wait_for_tx` - Wait for the transaction to be mined.
/// * `show_receipt` - If the receipt of the transaction should be displayed on stdout.
pub async fn handle_transaction_result<P>(
    provider: P,
    transaction_result: InvokeTransactionResult,
    wait_for_tx: bool,
    show_receipt: bool,
) -> Result<()>
where
    P: Provider + Send,
{
    println!("Transaction hash: {:#x}", transaction_result.transaction_hash);

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
