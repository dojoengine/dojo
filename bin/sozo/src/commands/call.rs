use anyhow::{anyhow, Result};
use clap::Args;
use dojo_types::naming;
use scarb::core::Config;
use sozo_ops::resource_descriptor::ResourceDescriptor;
use sozo_scarbext::WorkspaceExt;
use starknet::core::types::{BlockId, BlockTag, FunctionCall, StarknetError};
use starknet::core::utils as snutils;
use starknet::providers::{Provider, ProviderError};
use tracing::trace;

use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::commands::calldata_decoder;
use crate::utils;

#[derive(Debug, Args)]
#[command(about = "Call a system with the given calldata.")]
pub struct CallArgs {
    #[arg(help = "The tag or address of the contract to call.")]
    pub tag_or_address: ResourceDescriptor,

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

        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

        let profile_config = ws.load_profile_config()?;

        let descriptor = self.tag_or_address.ensure_namespace(&profile_config.namespace.default);

        config.tokio_handle().block_on(async {
            let (world_diff, provider, _) =
                utils::get_world_diff_and_provider(self.starknet.clone(), self.world, &ws).await?;

            let calldata = if let Some(cd) = self.calldata {
                calldata_decoder::decode_calldata(&cd)?
            } else {
                vec![]
            };

            let contract_address = match &descriptor {
                ResourceDescriptor::Address(address) => Some(*address),
                ResourceDescriptor::Tag(tag) => {
                    let selector = naming::compute_selector_from_tag(tag);
                    world_diff.get_contract_address(selector)
                }
                ResourceDescriptor::Name(_) => {
                    unimplemented!("Expected to be a resolved tag with default namespace.")
                }
            }
            .ok_or_else(|| anyhow!("Contract {descriptor} not found in the world diff."))?;

            let block_id = if let Some(block_id) = self.block_id {
                dojo_utils::parse_block_id(block_id)?
            } else {
                BlockId::Tag(BlockTag::Pending)
            };

            let res = provider
                .call(
                    FunctionCall {
                        contract_address,
                        entry_point_selector: snutils::get_selector_from_name(&self.entrypoint)?,
                        calldata,
                    },
                    block_id,
                )
                .await;

            match res {
                Ok(output) => {
                    println!(
                        "[ {} ]",
                        output.iter().map(|o| format!("0x{:x}", o)).collect::<Vec<_>>().join(" ")
                    );
                }
                Err(e) => {
                    anyhow::bail!(format!(
                        "Error calling entrypoint `{}` on address: {:#066x}\n{}",
                        self.entrypoint,
                        contract_address,
                        match &e {
                            ProviderError::StarknetError(StarknetError::ContractError(e)) => {
                                format!("Contract error: {}", e.revert_error.clone())
                            }
                            _ => e.to_string(),
                        }
                    ));
                }
            };

            Ok(())
        })
    }
}
