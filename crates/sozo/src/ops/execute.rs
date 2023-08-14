use anyhow::{Context, Result};
use dojo_client::contract::world::WorldContract;
use dojo_world::environment::Environment;

use crate::commands::execute::ExecuteArgs;

pub async fn execute(args: ExecuteArgs, env_metadata: Option<Environment>) -> Result<()> {
    let ExecuteArgs { system, calldata, world, starknet, account } = args;

    let world_address = world.address(env_metadata.as_ref())?;
    let provider = starknet.provider(env_metadata.as_ref())?;

    let account = account.account(provider, env_metadata.as_ref()).await?;
    let world = WorldContract::new(world_address, &account);

    let res =
        world.execute(&system, calldata).await.with_context(|| "Failed to send transaction")?;

    println!("Transaction: {:#x}", res.transaction_hash);

    Ok(())
}
