use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};
use clap::Args;
use dojo_world::config::calldata_decoder;
use dojo_world::contracts::ContractInfo;
use scarb::core::Config;
use sozo_ops::resource_descriptor::ResourceDescriptor;
use sozo_scarbext::WorkspaceExt;
use starknet::core::types::{BlockId, BlockTag, FunctionCall, StarknetError};
use starknet::core::utils as snutils;
use starknet::providers::{Provider, ProviderError};
use tracing::trace;

use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::utils::{self, CALLDATA_DOC};

#[derive(Debug, Args)]
#[command(about = "Call a system with the given calldata.")]
pub struct CallArgs {
    #[arg(help = "* The tag or address of the Dojo contract to call OR,
* The address or the instance name of the Starknet contract to call OR,
* 'world' to call the Dojo world.")]
    pub tag_or_address: ResourceDescriptor,

    #[arg(help = "The name of the entrypoint to call.")]
    pub entrypoint: String,

    #[arg(num_args = 0..)]
    #[arg(help = format!("The calldata to be passed to the system.
{CALLDATA_DOC}"))]
    pub calldata: Vec<String>,

    #[arg(short, long)]
    #[arg(help = "The block ID (could be a hash, a number, 'pending' or 'latest')")]
    pub block_id: Option<String>,

    #[arg(long)]
    #[arg(help = "If true, sozo will compute the diff of the world from the chain to translate \
                  tags to addresses.")]
    pub diff: bool,

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

        let CallArgs { tag_or_address, .. } = self;

        config.tokio_handle().block_on(async {
            let descriptor =
                tag_or_address.clone().ensure_namespace(&profile_config.namespace.default);

            let local_manifest = ws.read_manifest_profile()?;

            let calldata = calldata_decoder::decode_calldata(&self.calldata)?;

            let contracts: HashMap<String, ContractInfo> = if self.diff || local_manifest.is_none()
            {
                let (world_diff, _, _) =
                    utils::get_world_diff_and_provider(self.starknet.clone(), self.world, &ws)
                        .await?;

                (&world_diff).into()
            } else {
                match &local_manifest {
                    Some(manifest) => manifest.into(),
                    _ => bail!(
                        "Unable to get the list of contracts, either from the world or from the \
                         local manifest."
                    ),
                }
            };

            let mut contract_address = match &descriptor {
                ResourceDescriptor::Address(address) => Some(*address),
                ResourceDescriptor::Tag(tag) => {
                    // Try to find the contract to call among Dojo contracts
                    contracts.get(tag).map(|c| c.address)
                }
                ResourceDescriptor::Name(_) => {
                    unimplemented!("Expected to be a resolved tag with default namespace.")
                }
            };

            if contract_address.is_none() {
                contract_address = match &tag_or_address {
                    ResourceDescriptor::Name(name) => contracts.get(name).map(|c| c.address),
                    ResourceDescriptor::Address(_) | ResourceDescriptor::Tag(_) => {
                        // A contract should have already been found while searching for a Dojo
                        // contract.
                        None
                    }
                }
            }

            let contract_address = contract_address
                .ok_or_else(|| anyhow!("Contract {descriptor} not found in the world diff."))?;

            let block_id = if let Some(block_id) = self.block_id {
                dojo_utils::parse_block_id(block_id)?
            } else {
                BlockId::Tag(BlockTag::Pending)
            };

            let (provider, _) = self.starknet.provider(profile_config.env.as_ref())?;

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
