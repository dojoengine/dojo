use anyhow::Result;
use clap::Args;
use dojo_world::metadata::dojo_metadata_from_workspace;
use scarb::core::Config;
use tracing::trace;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;

#[derive(Debug, Args)]
pub struct PrintEnvArgs {
    #[command(flatten)]
    account: AccountOptions,

    #[command(flatten)]
    starknet: StarknetOptions,

    #[command(flatten)]
    world: WorldOptions,
}

impl PrintEnvArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(args = ?self);
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        let ui = ws.config().ui();

        let dojo_metadata = dojo_metadata_from_workspace(&ws)?;

        let env_metadata = if config.manifest_path().exists() {
            dojo_metadata.env().cloned()
        } else {
            trace!("Manifest path does not exist.");
            None
        };

        let PrintEnvArgs { world, account, starknet } = self;

        if let Ok(world_address) = world.address(env_metadata.as_ref()) {
            ui.print(format!("World address: {world_address:#064x}"));
        }

        if let Ok(account_address) = account.account_address(env_metadata.as_ref()) {
            ui.print(format!("Account address: {account_address:#064x}"));
        }

        if account.signer.private_key(env_metadata.as_ref()).is_some() {
            ui.print("Private key is set. It will take precedence over keystore".to_string())
        } else if let Some(keystore_path) = account.signer.keystore_path(env_metadata.as_ref()) {
            ui.print(format!("Keystore Path: {keystore_path}"));
        }

        if let Ok(rpc_url) = starknet.url(env_metadata.as_ref()) {
            ui.print(format!("RPC URL: {rpc_url}"));
        }

        Ok(())
    }
}
