use anyhow::Result;
use clap::Args;
use sozo_ui::SozoUi;
use starknet::core::types::requests::{
    GetBlockWithReceiptsRequest, GetBlockWithTxHashesRequest, GetBlockWithTxsRequest,
};
use starknet::core::types::BlockId;
use starknet::providers::{Provider, ProviderRequestData, ProviderResponseData};
use tracing::trace;

use super::{print_json, BlockIdOption, OutputOptions};
use crate::commands::options::starknet::StarknetOptions;

#[derive(Debug, Args)]
#[command(about = "Get the latest block number")]
pub struct BlockNumberArgs {
    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl BlockNumberArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let (provider, _) = self.starknet.provider(None)?;
        let block_number = provider.block_number().await?;

        print_json(
            ui,
            &serde_json::json!({
                "block_number": block_number
            }),
            self.output.raw,
        )
    }
}

#[derive(Debug, Args)]
#[command(about = "Get the latest block hash")]
pub struct BlockHashArgs {
    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl BlockHashArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let (provider, _) = self.starknet.provider(None)?;
        let result = provider.block_hash_and_number().await?;

        print_json(
            ui,
            &serde_json::json!({
                "block_hash": format!("{:#066x}", result.block_hash),
                "block_number": result.block_number
            }),
            self.output.raw,
        )
    }
}

#[derive(Debug, Args)]
#[command(about = "Get block information")]
pub struct BlockArgs {
    #[arg(help = "Block ID(s) - number, hash, 'latest', 'pending'. Supports multiple for batching")]
    pub block_ids: Vec<String>,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[arg(long, help = "Include full transaction details")]
    pub full: bool,

    #[arg(long, help = "Include transaction receipts")]
    pub receipts: bool,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl BlockArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let (provider, _) = self.starknet.provider(None)?;

        // If no block IDs provided, default to latest
        let block_ids: Vec<BlockId> = if self.block_ids.is_empty() {
            vec![BlockId::Tag(starknet::core::types::BlockTag::Latest)]
        } else {
            self.block_ids
                .iter()
                .map(|id| dojo_utils::parse_block_id(id.clone()))
                .collect::<Result<Vec<_>>>()?
        };

        if block_ids.len() == 1 {
            // Single request (existing behavior)
            let block_id = block_ids[0].clone();
            if self.receipts {
                let block = provider.get_block_with_receipts(block_id).await?;
                print_json(ui, &block, self.output.raw)
            } else if self.full {
                let block = provider.get_block_with_txs(block_id).await?;
                print_json(ui, &block, self.output.raw)
            } else {
                let block = provider.get_block_with_tx_hashes(block_id).await?;
                print_json(ui, &block, self.output.raw)
            }
        } else {
            // Batch request
            let requests: Vec<ProviderRequestData> = block_ids
                .iter()
                .map(|block_id| {
                    if self.receipts {
                        ProviderRequestData::GetBlockWithReceipts(GetBlockWithReceiptsRequest {
                            block_id: block_id.clone(),
                        })
                    } else if self.full {
                        ProviderRequestData::GetBlockWithTxs(GetBlockWithTxsRequest {
                            block_id: block_id.clone(),
                        })
                    } else {
                        ProviderRequestData::GetBlockWithTxHashes(GetBlockWithTxHashesRequest {
                            block_id: block_id.clone(),
                        })
                    }
                })
                .collect();

            let responses = provider.batch_requests(&requests).await?;

            // Extract block data from responses
            let blocks: Vec<serde_json::Value> = responses
                .into_iter()
                .map(|r| match r {
                    ProviderResponseData::GetBlockWithTxHashes(block) => {
                        serde_json::to_value(block).unwrap()
                    }
                    ProviderResponseData::GetBlockWithTxs(block) => {
                        serde_json::to_value(block).unwrap()
                    }
                    ProviderResponseData::GetBlockWithReceipts(block) => {
                        serde_json::to_value(block).unwrap()
                    }
                    _ => panic!("Unexpected response type"),
                })
                .collect();

            print_json(ui, &blocks, self.output.raw)
        }
    }
}

#[derive(Debug, Args)]
#[command(about = "Get block timestamp")]
pub struct BlockTimeArgs {
    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub block_id: BlockIdOption,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl BlockTimeArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let (provider, _) = self.starknet.provider(None)?;
        let block_id = self.block_id.to_block_id()?;

        let block = provider.get_block_with_tx_hashes(block_id).await?;

        // Extract timestamp from the JSON representation
        let block_json = serde_json::to_value(&block)?;
        let timestamp = block_json
            .get("timestamp")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        print_json(
            ui,
            &serde_json::json!({
                "timestamp": timestamp,
                "datetime": format_timestamp(timestamp)
            }),
            self.output.raw,
        )
    }
}

/// Format a unix timestamp as RFC3339 datetime string.
fn format_timestamp(timestamp: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};

    let time = UNIX_EPOCH + Duration::from_secs(timestamp);
    if let Ok(duration) = time.duration_since(UNIX_EPOCH) {
        // Calculate components manually
        let secs = duration.as_secs();
        // days since epoch
        let days = secs / 86400;
        let remaining = secs % 86400;
        let hours = remaining / 3600;
        let minutes = (remaining % 3600) / 60;
        let seconds = remaining % 60;

        // Calculate year/month/day from days since epoch
        let mut year = 1970;
        let mut day_count = days as i64;

        loop {
            let days_in_year = if is_leap_year(year) { 366 } else { 365 };
            if day_count < days_in_year {
                break;
            }
            day_count -= days_in_year;
            year += 1;
        }

        // Find month and day
        let days_in_months: [i64; 12] = if is_leap_year(year) {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };

        let mut month = 1;
        for &days_in_month in &days_in_months {
            if day_count < days_in_month {
                break;
            }
            day_count -= days_in_month;
            month += 1;
        }
        let day = day_count + 1;

        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            year, month, day, hours, minutes, seconds
        )
    } else {
        "invalid timestamp".to_string()
    }
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}
