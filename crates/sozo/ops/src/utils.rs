use anyhow::{anyhow, Result};
use bigdecimal::BigDecimal;
use cainome::cairo_serde::ClassHash;
use dojo_utils::{execution_status_from_receipt, TransactionWaiter};
use dojo_world::contracts::naming::get_name_from_tag;
use dojo_world::contracts::world::{WorldContract, WorldContractReader};
use dojo_world::migration::strategy::generate_salt;
use scarb_ui::Ui;
#[cfg(feature = "walnut")]
use sozo_walnut::WalnutDebugger;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{BlockId, BlockTag, ExecutionResult, Felt, InvokeTransactionResult};
use starknet::providers::Provider;

use crate::migration::ui::MigrationUi;

/// Retrieves a contract address from it's name
/// using the world's data, or parses a hex string into
/// a [`Felt`].
///
/// # Arguments
///
/// * `world` - The world's contract connector.
/// * `tag_or_address` - A string with a contract tag or a hexadecimal address.
///
/// # Returns
///
/// A [`Felt`] with the address of the contract on success.
pub async fn get_contract_address<A: ConnectedAccount + Sync>(
    world: &WorldContract<A>,
    tag_or_address: &str,
) -> Result<Felt> {
    if tag_or_address.starts_with("0x") {
        Felt::from_hex(tag_or_address).map_err(anyhow::Error::from)
    } else {
        // Use contract class hash -> using the original one from the migration.
        let contract_class_hash = ClassHash(Felt::ONE);

        Ok(starknet::core::utils::get_contract_address(
            generate_salt(&get_name_from_tag(tag_or_address)),
            contract_class_hash.into(),
            &[],
            world.address,
        ))
    }
}

/// Retrieves a contract address from its name
/// using a world contract reader, or parses a hex string into
/// a [`Felt`].
///
/// # Arguments
///
/// * `world_reader` - The world contract reader.
/// * `tag_or_address` - A string with a contract tag or a hexadecimal address.
///
/// # Returns
///
/// A [`Felt`] with the address of the contract on success.
pub async fn get_contract_address_from_reader<P: Provider + Sync + Send>(
    world_reader: &WorldContractReader<P>,
    tag_or_address: String,
) -> Result<Felt> {
    if tag_or_address.starts_with("0x") {
        Felt::from_hex(&tag_or_address).map_err(anyhow::Error::from)
    } else {
        let class_hash = ClassHash(Felt::ONE);
        Ok(starknet::core::utils::get_contract_address(
            generate_salt(&get_name_from_tag(&tag_or_address)),
            class_hash.into(),
            &[],
            world_reader.address,
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
/// * `walnut_debugger` - Optionally a Walnut debugger to debug the transaction. stdout.
pub async fn handle_transaction_result<P>(
    ui: &Ui,
    provider: P,
    transaction_result: InvokeTransactionResult,
    wait_for_tx: bool,
    show_receipt: bool,
    #[cfg(feature = "walnut")] walnut_debugger: &Option<WalnutDebugger>,
) -> Result<()>
where
    P: Provider + Send,
{
    ui.print_sub(format!("Transaction hash: {:#066x}", transaction_result.transaction_hash));

    if wait_for_tx {
        let receipt =
            TransactionWaiter::new(transaction_result.transaction_hash, &provider).await?;

        if show_receipt {
            ui.print_sub(format!("Receipt:\n{}", serde_json::to_string_pretty(&receipt)?));
        } else {
            match execution_status_from_receipt(&receipt.receipt) {
                ExecutionResult::Succeeded => {
                    ui.print_sub("Status: OK");
                }
                ExecutionResult::Reverted { reason } => {
                    ui.print_sub("Status: REVERTED");
                    ui.print(format!("Reason:\n{}", reason));
                }
            };

            #[cfg(feature = "walnut")]
            if let Some(walnut_debugger) = walnut_debugger {
                walnut_debugger.debug_transaction(ui, &transaction_result.transaction_hash)?;
            }
        }
    }

    Ok(())
}

/// Parses a string into a [`BlockId`].
///
/// # Arguments
///
/// * `block_str` - a string representing a block ID. It could be a block hash starting with 0x, a
///   block number, 'pending' or 'latest'.
///
/// # Returns
///
/// The parsed [`BlockId`] on success.
pub fn parse_block_id(block_str: String) -> Result<BlockId> {
    if block_str.starts_with("0x") {
        let hash = Felt::from_hex(&block_str)
            .map_err(|_| anyhow!("Unable to parse block hash: {}", block_str))?;
        Ok(BlockId::Hash(hash))
    } else if block_str.eq("pending") {
        Ok(BlockId::Tag(BlockTag::Pending))
    } else if block_str.eq("latest") {
        Ok(BlockId::Tag(BlockTag::Latest))
    } else {
        match block_str.parse::<u64>() {
            Ok(n) => Ok(BlockId::Number(n)),
            Err(_) => Err(anyhow!("Unable to parse block ID: {}", block_str)),
        }
    }
}

/// Convert a [`Felt`] into a [`BigDecimal`] with a given number of decimals.
pub fn felt_to_bigdecimal<F, D>(felt: F, decimals: D) -> BigDecimal
where
    F: AsRef<Felt>,
    D: Into<i64>,
{
    BigDecimal::from((felt.as_ref().to_bigint(), decimals.into()))
}
