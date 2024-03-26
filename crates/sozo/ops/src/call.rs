use anyhow::{Context, Result};
use dojo_world::contracts::world::WorldContract;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{BlockId, BlockTag, FieldElement, FunctionCall};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;

use super::get_contract_address;

pub async fn call<A>(
    contract: String,
    entrypoint: String,
    calldata: Vec<FieldElement>,
    world: WorldContract<A>,
) -> Result<()>
where
    A: ConnectedAccount + Sync + Send + 'static,
{
    let contract_address = get_contract_address(&world, contract).await?;
    let output = world
        .account
        .provider()
        .call(
            FunctionCall {
                contract_address,
                entry_point_selector: get_selector_from_name(&entrypoint)?,
                calldata,
            },
            BlockId::Tag(BlockTag::Latest),
        )
        .await
        .with_context(|| format!("Failed to call {entrypoint}"))?;

    println!("[ {} ]", output.iter().map(|o| format!("0x{:x}", o)).collect::<Vec<_>>().join(" "));

    Ok(())
}
