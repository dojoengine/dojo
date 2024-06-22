use anyhow::Result;
use clap::Args;
use scarb::core::Config;
use sozo_ops::execute;
use tracing::trace;

use super::calldata_decoder;
use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Debug, Args)]
#[command(about = "Execute a system with the given calldata.")]
pub struct ExecuteArgs {
    #[arg(help = "The address of the contract to be executed. Or fully qualified contract name \
                  (ex: dojo_example::actions::actions")]
    pub contract: String,

    #[arg(help = "The name of the entrypoint to be executed.")]
    pub entrypoint: String,

    #[arg(short, long)]
    #[arg(help = "The calldata to be passed to the system. Comma separated values e.g., \
                  0x12345,128,u256:9999999999. Sozo supports some prefixes that you can use to \
                  automatically parse some types. The supported prefixes are:
                  - u256: A 256-bit unsigned integer.
                  - sstr: A cairo short string.
                  - str: A cairo string (ByteArray).
                  - no prefix: A cairo felt or any type that fit into one felt.")]
    pub calldata: Option<String>,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub account: AccountOptions,

    #[command(flatten)]
    pub world: WorldOptions,

    #[command(flatten)]
    pub transaction: TransactionOptions,
}

impl ExecuteArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(args = ?self);
        let env_metadata = utils::load_metadata_from_config(config)?;

        config.tokio_handle().block_on(async {
            let world = utils::world_from_env_metadata(
                self.world,
                self.account,
                self.starknet,
                &env_metadata,
                config,
            )
            .await?;

            let tx_config = self.transaction.into();

            trace!(
                contract=?self.contract,
                entrypoint=self.entrypoint,
                calldata=?self.calldata,
                "Executing Execute command."
            );

            let calldata = if let Some(cd) = self.calldata {
                calldata_decoder::decode_calldata(&cd)?
            } else {
                vec![]
            };

            execute::execute(
                &config.ui(),
                self.contract,
                self.entrypoint,
                calldata,
                &world,
                &tx_config,
            )
            .await
        })
    }
}
