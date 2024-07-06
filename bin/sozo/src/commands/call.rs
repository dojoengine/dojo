use anyhow::Result;
use clap::Args;
use dojo_world::contracts::naming::ensure_namespace;
use dojo_world::metadata::get_default_namespace_from_ws;
use scarb::core::Config;
use starknet::core::types::Felt;
use tracing::trace;

use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Debug, Args)]
#[command(about = "Call a system with the given calldata.")]
pub struct CallArgs {
    #[arg(help = "The tag or address of the contract to call.")]
    pub tag_or_address: String,

    #[arg(help = "The name of the entrypoint to call.")]
    pub entrypoint: String,

    #[arg(short, long)]
    #[arg(value_delimiter = ',')]
    #[arg(help = "The calldata to be passed to the entrypoint. Comma separated values e.g., \
                  0x12345,0x69420.")]
    pub calldata: Vec<Felt>,

    #[arg(short, long)]
    #[arg(help = "The block ID (could be a hash, a number, 'pending' or 'latest')")]
    pub block_id: Option<String>,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub world: WorldOptions,
}

impl CallArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(args = ?self);

        let env_metadata = utils::load_metadata_from_config(config)?;
        trace!(?env_metadata, "Loaded metadata from config.");

        let tag_or_address = if utils::is_address(&self.tag_or_address) {
            self.tag_or_address
        } else {
            let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
            let default_namespace = get_default_namespace_from_ws(&ws);
            ensure_namespace(&self.tag_or_address, &default_namespace)
        };

        config.tokio_handle().block_on(async {
            let world_reader =
                utils::world_reader_from_env_metadata(self.world, self.starknet, &env_metadata)
                    .await
                    .unwrap();

            sozo_ops::call::call(
                world_reader,
                tag_or_address,
                self.entrypoint,
                self.calldata,
                self.block_id,
            )
            .await
        })
    }
}
