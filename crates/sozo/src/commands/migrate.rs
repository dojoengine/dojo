use anyhow::{Context, Result};
use clap::Args;
use scarb::core::Config;

use super::options::account::AccountOptions;
use super::options::dojo_metadata_from_workspace;
use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::ops::migration;

#[derive(Args)]
pub struct MigrateArgs {
    #[clap(short, long)]
    #[clap(help = "Perform a dry run and outputs the plan to be executed")]
    plan: bool,

    #[command(flatten)]
    world: WorldOptions,

    #[command(flatten)]
    starknet: StarknetOptions,

    #[command(flatten)]
    account: AccountOptions,
}

impl MigrateArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

        let target_dir = ws.target_dir().path_existent().unwrap();
        let target_dir = target_dir.join(ws.config().profile().as_str());

        if !target_dir.join("manifest.json").exists() {
            scarb::ops::compile(&ws)?;
        }

        let mut env_metadata = dojo_metadata_from_workspace(&ws)
            .and_then(|dojo_metadata| dojo_metadata.get("env").cloned());

        // If there is an environment-specific metadata, use that, otherwise use the
        // workspace's default environment metadata.
        env_metadata = env_metadata
            .as_ref()
            .and_then(|env_metadata| env_metadata.get(ws.config().profile().as_str()).cloned())
            .or(env_metadata);

        ws.config().tokio_handle().block_on(async {
            let world_address = self.world.address(env_metadata.as_ref()).ok();
            let provider = self.starknet.provider(env_metadata.as_ref())?;

            let account = self
                .account
                .account(provider, env_metadata.as_ref())
                .await
                .with_context(|| "Problem initializing account for migration.")?;

            migration::execute(world_address, account, target_dir, ws.config()).await
        })?;

        Ok(())
    }
}
