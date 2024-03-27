use anyhow::{anyhow, Context, Result};
use dojo_world::contracts::world::WorldContract;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{BlockId, BlockTag, FieldElement, FunctionCall};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;

use super::get_contract_address;

#[cfg(test)]
#[path = "call_test.rs"]
mod call_test;

pub fn parse_block_id(block_id: Option<String>) -> Result<BlockId> {
    if let Some(block_id) = block_id {
        if block_id.starts_with("0x") {
            let hash = FieldElement::from_hex_be(&block_id)
                .map_err(|_| anyhow!("Unable to parse block hash: {}", block_id))?;
            Ok(BlockId::Hash(hash))
        } else if block_id.eq("pending") {
            Ok(BlockId::Tag(BlockTag::Pending))
        } else if block_id.eq("latest") {
            Ok(BlockId::Tag(BlockTag::Latest))
        } else {
            match block_id.parse::<u64>() {
                Ok(n) => Ok(BlockId::Number(n)),
                Err(_) => Err(anyhow!("Unable to parse block ID: {}", block_id)),
            }
        }
    } else {
        Ok(BlockId::Tag(BlockTag::Pending))
    }
}

pub async fn call<A>(
    contract: String,
    entrypoint: String,
    calldata: Vec<FieldElement>,
    world: WorldContract<A>,
    block_id: Option<String>,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
{
    let contract_address = get_contract_address(&world, contract).await?;
    let block_id = parse_block_id(block_id)?;

    let output = world
        .account
        .provider()
        .call(
            FunctionCall {
                contract_address,
                entry_point_selector: get_selector_from_name(&entrypoint)?,
                calldata,
            },
            block_id,
        )
        .await
        .with_context(|| format!("Failed to call {entrypoint}"))?;

    println!("[ {} ]", output.iter().map(|o| format!("0x{:x}", o)).collect::<Vec<_>>().join(" "));

    Ok(())
}
