use anyhow::{Context, Result};
use dojo_world::metadata::Environment;
use starknet::accounts::{Account, Call};
use starknet::core::utils::get_selector_from_name;

use crate::commands::execute::ExecuteArgs;

pub async fn execute(args: ExecuteArgs, env_metadata: Option<Environment>) -> Result<()> {
    let ExecuteArgs { contract, entrypoint, calldata, starknet, account } = args;

    let provider = starknet.provider(env_metadata.as_ref())?;

    let account = account.account(provider, env_metadata.as_ref()).await?;

    let res = account
        .execute(vec![Call {
            calldata,
            to: contract,
            selector: get_selector_from_name(&entrypoint).unwrap(),
        }])
        .send()
        .await
        .with_context(|| "Failed to send transaction")?;

    println!("Transaction: {:#x}", res.transaction_hash);

    Ok(())
}
