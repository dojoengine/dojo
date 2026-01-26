use anyhow::Result;
use clap::Args;
use sozo_ui::SozoUi;
use starknet::core::types::{Felt, FunctionCall};
use starknet::core::utils::get_selector_from_name;
use starknet::macros::felt;
use starknet::providers::Provider;
use tracing::trace;

use super::{parse_felt, print_json, BlockIdOption, OutputOptions};
use crate::commands::options::starknet::StarknetOptions;

/// ETH token contract address on Starknet.
const ETH_CONTRACT_ADDRESS: Felt =
    felt!("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7");

/// STRK token contract address on Starknet.
const STRK_CONTRACT_ADDRESS: Felt =
    felt!("0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d");

#[derive(Debug, Args)]
#[command(about = "Get STRK or ETH balance of an address")]
pub struct BalanceArgs {
    #[arg(help = "The address to check balance for (hex or decimal)")]
    pub address: String,

    #[arg(long, help = "Get ETH balance instead of STRK")]
    pub eth: bool,

    #[arg(long, help = "Custom ERC20 token contract address")]
    pub token: Option<String>,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub block_id: BlockIdOption,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl BalanceArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let address = parse_felt(&self.address)?;
        let (provider, _) = self.starknet.provider(None)?;
        let block_id = self.block_id.to_block_id()?;

        let token_address = if let Some(token) = &self.token {
            parse_felt(token)?
        } else if self.eth {
            ETH_CONTRACT_ADDRESS
        } else {
            STRK_CONTRACT_ADDRESS
        };

        let token_name = if self.token.is_some() {
            "Custom Token"
        } else if self.eth {
            "ETH"
        } else {
            "STRK"
        };

        let balance = provider
            .call(
                FunctionCall {
                    contract_address: token_address,
                    entry_point_selector: get_selector_from_name("balanceOf")?,
                    calldata: vec![address],
                },
                block_id,
            )
            .await?;

        // Balance is returned as u256 (low, high)
        let balance_low = balance.first().copied().unwrap_or(Felt::ZERO);
        let balance_high = balance.get(1).copied().unwrap_or(Felt::ZERO);

        let wei = format_u256(balance_low, balance_high);
        let formatted = format_balance_with_decimals(balance_low, balance_high, 18);

        print_json(
            ui,
            &serde_json::json!({
                "address": format!("{:#066x}", address),
                "token": token_name,
                "balance_low": format!("{:#066x}", balance_low),
                "balance_high": format!("{:#066x}", balance_high),
                "balance_wei": wei,
                "balance_formatted": formatted
            }),
            self.output.raw,
        )
    }
}

/// Format a u256 (low, high) as a string.
/// Returns decimal for small values, hex for larger ones.
fn format_u256(low: Felt, high: Felt) -> String {
    // If high is zero, we can convert low to u128 directly
    if high == Felt::ZERO {
        // Try to convert to u128 for decimal representation
        let bytes = low.to_bytes_be();
        // Check if it fits in u128 (first 16 bytes should be zero)
        if bytes[..16].iter().all(|&b| b == 0) {
            let mut u128_bytes = [0u8; 16];
            u128_bytes.copy_from_slice(&bytes[16..]);
            let value = u128::from_be_bytes(u128_bytes);
            return value.to_string();
        }
    }

    // For larger values, output as two hex values
    format!("{:#066x}:{:#066x}", high, low)
}

/// Format a u256 balance with decimals.
fn format_balance_with_decimals(low: Felt, high: Felt, decimals: u32) -> String {
    let wei_str = format_u256(low, high);

    // If the format contains a colon, it's a large value - return as is
    if wei_str.contains(':') {
        return wei_str;
    }

    // If the number is smaller than 10^decimals, pad with zeros
    if wei_str.len() <= decimals as usize {
        let zeros_needed = decimals as usize - wei_str.len();
        return format!("0.{}{}", "0".repeat(zeros_needed), wei_str);
    }

    let split_point = wei_str.len() - decimals as usize;
    let (integer, fraction) = wei_str.split_at(split_point);

    // Trim trailing zeros from fraction
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        integer.to_string()
    } else {
        format!("{}.{}", integer, fraction)
    }
}

#[derive(Debug, Args)]
#[command(about = "Get the nonce of an address")]
pub struct NonceArgs {
    #[arg(help = "The contract address (hex or decimal)")]
    pub address: String,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub block_id: BlockIdOption,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl NonceArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let address = parse_felt(&self.address)?;
        let (provider, _) = self.starknet.provider(None)?;
        let block_id = self.block_id.to_block_id()?;

        let nonce = provider.get_nonce(block_id, address).await?;

        print_json(
            ui,
            &serde_json::json!({
                "address": format!("{:#066x}", address),
                "nonce": self.output.format_felt(nonce)
            }),
            self.output.raw,
        )
    }
}

#[derive(Debug, Args)]
#[command(about = "Get storage value at a key for a contract")]
pub struct StorageArgs {
    #[arg(help = "The contract address (hex or decimal)")]
    pub address: String,

    #[arg(help = "The storage key (hex or decimal)")]
    pub key: String,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub block_id: BlockIdOption,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl StorageArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let address = parse_felt(&self.address)?;
        let key = parse_felt(&self.key)?;
        let (provider, _) = self.starknet.provider(None)?;
        let block_id = self.block_id.to_block_id()?;

        let value = provider.get_storage_at(address, key, block_id).await?;

        print_json(
            ui,
            &serde_json::json!({
                "address": format!("{:#066x}", address),
                "key": format!("{:#066x}", key),
                "value": self.output.format_felt(value)
            }),
            self.output.raw,
        )
    }
}

#[derive(Debug, Args)]
#[command(about = "Get the class hash at a contract address")]
pub struct ClassHashAtArgs {
    #[arg(help = "The contract address (hex or decimal)")]
    pub address: String,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub block_id: BlockIdOption,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl ClassHashAtArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let address = parse_felt(&self.address)?;
        let (provider, _) = self.starknet.provider(None)?;
        let block_id = self.block_id.to_block_id()?;

        let class_hash = provider.get_class_hash_at(block_id, address).await?;

        print_json(
            ui,
            &serde_json::json!({
                "address": format!("{:#066x}", address),
                "class_hash": format!("{:#066x}", class_hash)
            }),
            self.output.raw,
        )
    }
}

#[derive(Debug, Args)]
#[command(about = "Get the class at a contract address")]
pub struct ClassAtArgs {
    #[arg(help = "The contract address (hex or decimal)")]
    pub address: String,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub block_id: BlockIdOption,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl ClassAtArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let address = parse_felt(&self.address)?;
        let (provider, _) = self.starknet.provider(None)?;
        let block_id = self.block_id.to_block_id()?;

        let class = provider.get_class_at(block_id, address).await?;

        print_json(ui, &class, self.output.raw)
    }
}

#[derive(Debug, Args)]
#[command(about = "Get class by its hash")]
pub struct ClassByHashArgs {
    #[arg(help = "The class hash (hex or decimal)")]
    pub class_hash: String,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub block_id: BlockIdOption,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl ClassByHashArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let class_hash = parse_felt(&self.class_hash)?;
        let (provider, _) = self.starknet.provider(None)?;
        let block_id = self.block_id.to_block_id()?;

        let class = provider.get_class(block_id, class_hash).await?;

        print_json(ui, &class, self.output.raw)
    }
}
