use anyhow::{Context, Result};
use dojo_world::contracts::world::WorldContract;
use dojo_world::metadata::Environment;
use dojo_world::utils::TransactionWaiter;
use starknet::accounts::{Account, Call};
use starknet::core::utils::get_selector_from_name;

use super::get_contract_address;
use crate::commands::execute::ExecuteArgs;

pub async fn execute(args: ExecuteArgs, env_metadata: Option<Environment>) -> Result<()> {
    let ExecuteArgs { contract, entrypoint, calldata, starknet, world, account, transaction } =
        args;

    let provider = starknet.provider(env_metadata.as_ref())?;

    let account = account.account(&provider, env_metadata.as_ref()).await?;
    let world_address = world.address(env_metadata.as_ref())?;
    let world = WorldContract::new(world_address, &account);

    let contract_address = get_contract_address(&world, contract).await?;
    let res = account
        .execute(vec![Call {
            calldata,
            to: contract_address,
            selector: get_selector_from_name(&entrypoint)?,
        }])
        .send()
        .await
        .with_context(|| "Failed to send transaction")?;

    if transaction.wait {
        let receipt = TransactionWaiter::new(res.transaction_hash, &provider).await?;
        println!("{}", serde_json::to_string_pretty(&receipt)?);
    } else {
        println!("Transaction hash: {:#x}", res.transaction_hash);
    }

    Ok(())
}
