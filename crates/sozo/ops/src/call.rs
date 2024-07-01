use anyhow::{Context, Result};
use dojo_world::contracts::WorldContractReader;
use starknet::core::types::{BlockId, BlockTag, Felt, FunctionCall};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;

use crate::utils::{get_contract_address_from_reader, parse_block_id};

pub async fn call<P: Provider + Sync + Send>(
    world_reader: WorldContractReader<P>,
    contract: String,
    entrypoint: String,
    calldata: Vec<Felt>,
    block_id: Option<String>,
) -> Result<()> {
    let contract_address = get_contract_address_from_reader(&world_reader, contract).await?;
    let block_id = if let Some(block_id) = block_id {
        parse_block_id(block_id)?
    } else {
        BlockId::Tag(BlockTag::Pending)
    };

    let output = world_reader
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
