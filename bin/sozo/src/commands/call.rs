use anyhow::Result;
use clap::Args;
use scarb::core::Config;
use starknet::core::types::FieldElement;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Debug, Args)]
#[command(about = "Call a system with the given calldata.")]
pub struct CallArgs {
    #[arg(help = "The address or the fully qualified name of the contract to call.")]
    pub contract: String,

    #[arg(help = "The name of the entrypoint to call.")]
    pub entrypoint: String,

    #[arg(short, long)]
    #[arg(value_delimiter = ',')]
    #[arg(help = "The calldata to be passed to the entrypoint. Comma separated values e.g., \
                  0x12345,0x69420.")]
    pub calldata: Vec<FieldElement>,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub account: AccountOptions,

    #[command(flatten)]
    pub world: WorldOptions,
}

impl CallArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let env_metadata = utils::load_metadata_from_config(config)?;

        config.tokio_handle().block_on(async {
            let world = utils::world_from_env_metadata(
                self.world,
                self.account,
                self.starknet,
                &env_metadata,
            )
            .await
            .unwrap();
            sozo_ops::call::call(self.contract, self.entrypoint, self.calldata, world).await
        })
    }
}
