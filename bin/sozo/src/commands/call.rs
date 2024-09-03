use anyhow::Result;
use clap::Args;
use dojo_world::contracts::naming::ensure_namespace;
use dojo_world::metadata::get_default_namespace_from_ws;
use scarb::core::Config;
use tracing::trace;

use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::commands::calldata_decoder;
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
                  0x12345,128,u256:9999999999. Sozo supports some prefixes that you can use to \
                  automatically parse some types. The supported prefixes are:
                  - u256: A 256-bit unsigned integer.
                  - sstr: A cairo short string.
                  - str: A cairo string (ByteArray).
                  - int: A signed integer.
                  - no prefix: A cairo felt or any type that fit into one felt.")]
    pub calldata: Option<String>,

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
            let default_namespace = get_default_namespace_from_ws(&ws)?;
            ensure_namespace(&self.tag_or_address, &default_namespace)
        };

        config.tokio_handle().block_on(async {
            let world_reader =
                utils::world_reader_from_env_metadata(self.world, self.starknet, &env_metadata)
                    .await
                    .unwrap();

            let calldata = if let Some(cd) = self.calldata {
                calldata_decoder::decode_calldata(&cd)?
            } else {
                vec![]
            };

            sozo_ops::call::call(
                &config.ui(),
                world_reader,
                tag_or_address,
                self.entrypoint,
                calldata,
                self.block_id,
            )
            .await
        })
    }
}
