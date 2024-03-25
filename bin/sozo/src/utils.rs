use anyhow::Error;
use dojo_world::contracts::world::WorldContract;
use dojo_world::metadata::{dojo_metadata_from_workspace, Environment};
use scarb::core::Config;
use starknet::accounts::SingleOwnerAccount;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::LocalWallet;

use crate::commands::options::account::AccountOptions;
use crate::commands::options::starknet::StarknetOptions;
use crate::commands::options::world::WorldOptions;

pub fn load_metadata_from_config(config: &Config) -> Result<Option<Environment>, Error> {
    let env_metadata = if config.manifest_path().exists() {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

        dojo_metadata_from_workspace(&ws).and_then(|inner| inner.env().cloned())
    } else {
        None
    };

    Ok(env_metadata)
}

pub async fn world_from_env_metadata(
    world: WorldOptions,
    account: AccountOptions,
    starknet: StarknetOptions,
    env_metadata: &Option<Environment>,
) -> Result<WorldContract<SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>>, Error> {
    let world_address = world.address(env_metadata.as_ref())?;
    let provider = starknet.provider(env_metadata.as_ref())?;

    let account = account.account(provider, env_metadata.as_ref()).await?;
    Ok(WorldContract::new(world_address, account))
}
