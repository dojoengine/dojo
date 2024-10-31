use anyhow::{anyhow, Result};
use bigdecimal::BigDecimal;
use cainome::cairo_serde::ClassHash;
use dojo_utils::{execution_status_from_receipt, TransactionWaiter};
use dojo_world::contracts::naming::get_name_from_tag;
use dojo_world::contracts::world::{WorldContract, WorldContractReader};
use dojo_world::migration::strategy::generate_salt;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{BlockId, BlockTag, ExecutionResult, Felt, InvokeTransactionResult};
use starknet::providers::Provider;

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
