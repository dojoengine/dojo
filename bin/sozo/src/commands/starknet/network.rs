use anyhow::Result;
use clap::Args;
use sozo_ui::SozoUi;
use starknet::core::types::SyncStatusType;
use starknet::providers::Provider;
use tracing::trace;

use super::{print_json, OutputOptions};
use crate::commands::options::starknet::StarknetOptions;

#[derive(Debug, Args)]
#[command(about = "Get the chain ID")]
pub struct ChainIdArgs {
    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl ChainIdArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let (provider, _) = self.starknet.provider(None)?;
        let chain_id = provider.chain_id().await?;

        // Try to decode as ASCII string for known chain IDs
        let chain_name = decode_chain_id(chain_id);

        print_json(
            ui,
            &serde_json::json!({
                "chain_id": format!("{:#066x}", chain_id),
                "chain_name": chain_name
            }),
            self.output.raw,
        )
    }
}

/// Decode a chain ID felt to its ASCII representation if possible.
fn decode_chain_id(chain_id: starknet::core::types::Felt) -> Option<String> {
    let bytes = chain_id.to_bytes_be();
    // Find first non-zero byte
    let start = bytes.iter().position(|&b| b != 0)?;
    let ascii_bytes = &bytes[start..];

    // Check if all bytes are valid ASCII
    if ascii_bytes.iter().all(|&b| b.is_ascii_graphic() || b.is_ascii_whitespace()) {
        String::from_utf8(ascii_bytes.to_vec()).ok()
    } else {
        None
    }
}

#[derive(Debug, Args)]
#[command(about = "Get the sync status of the node")]
pub struct SyncingArgs {
    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl SyncingArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let (provider, _) = self.starknet.provider(None)?;
        let sync_status = provider.syncing().await?;

        match &sync_status {
            SyncStatusType::NotSyncing => print_json(
                ui,
                &serde_json::json!({
                    "syncing": false,
                    "status": "fully synced"
                }),
                self.output.raw,
            ),
            SyncStatusType::Syncing(status) => {
                let total = status.highest_block_num - status.starting_block_num;
                let current = status.current_block_num - status.starting_block_num;
                let progress =
                    if total > 0 { (current as f64 / total as f64) * 100.0 } else { 0.0 };

                print_json(
                    ui,
                    &serde_json::json!({
                        "syncing": true,
                        "starting_block_hash": format!("{:#066x}", status.starting_block_hash),
                        "starting_block_num": status.starting_block_num,
                        "current_block_hash": format!("{:#066x}", status.current_block_hash),
                        "current_block_num": status.current_block_num,
                        "highest_block_hash": format!("{:#066x}", status.highest_block_hash),
                        "highest_block_num": status.highest_block_num,
                        "progress_percent": format!("{:.2}", progress)
                    }),
                    self.output.raw,
                )
            }
        }
    }
}
