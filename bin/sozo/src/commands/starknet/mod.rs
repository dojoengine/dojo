pub mod block;
pub mod network;
pub mod state;
pub mod transaction;

#[cfg(test)]
mod tests;

use anyhow::Result;
use clap::{Args, Subcommand};
use colored_json::ToColoredJson;
use sozo_ui::SozoUi;
use starknet::core::types::{BlockId, BlockTag, Felt};

/// Starknet utility commands that don't require a Dojo project context.
/// These commands provide direct interaction with the Starknet network.
#[derive(Debug, Args)]
pub struct StarknetArgs {
    #[command(subcommand)]
    command: StarknetCommand,
}

#[derive(Debug, Subcommand)]
pub enum StarknetCommand {
    #[command(about = "Get the latest block number")]
    BlockNumber(block::BlockNumberArgs),

    #[command(about = "Get the latest block hash")]
    BlockHash(block::BlockHashArgs),

    #[command(about = "Get block information")]
    Block(block::BlockArgs),

    #[command(about = "Get block timestamp")]
    BlockTime(block::BlockTimeArgs),

    #[command(alias = "tx", about = "Get transaction by hash (alias: tx)")]
    Transaction(transaction::TransactionArgs),

    #[command(about = "Get transaction receipt")]
    Receipt(transaction::ReceiptArgs),

    #[command(about = "Get transaction status")]
    Status(transaction::StatusArgs),

    #[command(about = "Get ETH or STRK balance of an address")]
    Balance(state::BalanceArgs),

    #[command(about = "Get the nonce of an address")]
    Nonce(state::NonceArgs),

    #[command(about = "Get storage value at a key for a contract")]
    Storage(state::StorageArgs),

    #[command(about = "Get the class hash at a contract address")]
    ClassHashAt(state::ClassHashAtArgs),

    #[command(about = "Get the class at a contract address")]
    ClassAt(state::ClassAtArgs),

    #[command(about = "Get class by its hash")]
    ClassByHash(state::ClassByHashArgs),

    #[command(about = "Get the chain ID")]
    ChainId(network::ChainIdArgs),

    #[command(about = "Get the sync status of the node")]
    Syncing(network::SyncingArgs),
}

impl StarknetArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        match self.command {
            StarknetCommand::BlockNumber(args) => args.run(ui).await,
            StarknetCommand::BlockHash(args) => args.run(ui).await,
            StarknetCommand::Block(args) => args.run(ui).await,
            StarknetCommand::BlockTime(args) => args.run(ui).await,
            StarknetCommand::Transaction(args) => args.run(ui).await,
            StarknetCommand::Receipt(args) => args.run(ui).await,
            StarknetCommand::Status(args) => args.run(ui).await,
            StarknetCommand::Balance(args) => args.run(ui).await,
            StarknetCommand::Nonce(args) => args.run(ui).await,
            StarknetCommand::Storage(args) => args.run(ui).await,
            StarknetCommand::ClassHashAt(args) => args.run(ui).await,
            StarknetCommand::ClassAt(args) => args.run(ui).await,
            StarknetCommand::ClassByHash(args) => args.run(ui).await,
            StarknetCommand::ChainId(args) => args.run(ui).await,
            StarknetCommand::Syncing(args) => args.run(ui).await,
        }
    }
}

/// Shared options for output formatting.
#[derive(Debug, Args, Clone)]
pub struct OutputOptions {
    #[arg(long, help = "Output raw JSON without colors")]
    pub raw: bool,

    #[arg(long, help = "Output numbers in decimal (default: hex)")]
    pub dec: bool,
}

impl OutputOptions {
    /// Format a Felt value according to output options.
    pub fn format_felt(&self, felt: Felt) -> String {
        if self.dec { felt.to_string() } else { format!("{:#066x}", felt) }
    }

    /// Format a u64 value according to output options.
    #[allow(dead_code)]
    pub fn format_u64(&self, value: u64) -> String {
        if self.dec { value.to_string() } else { format!("{:#x}", value) }
    }
}

/// Shared option for specifying a block ID.
#[derive(Debug, Args, Clone)]
pub struct BlockIdOption {
    #[arg(short = 'b', long, help = "Block ID (number, hash, 'latest', 'preconfirmed')")]
    pub block_id: Option<String>,
}

impl BlockIdOption {
    /// Parse the block ID option into a BlockId, defaulting to latest.
    pub fn to_block_id(&self) -> Result<BlockId> {
        if let Some(block_id) = &self.block_id {
            dojo_utils::parse_block_id(block_id.clone())
        } else {
            Ok(BlockId::Tag(BlockTag::Latest))
        }
    }
}

/// Parse a Felt from a hex or decimal string.
pub fn parse_felt(value: &str) -> Result<Felt> {
    if let Ok(felt) = Felt::from_hex(value) {
        return Ok(felt);
    }

    Felt::from_dec_str(value)
        .map_err(|_| anyhow::anyhow!("Invalid felt value `{value}`. Use hex (0x...) or decimal."))
}

/// Print JSON output with optional colorization.
/// By default, outputs colorized JSON. Use `--raw` to disable colors.
pub fn print_json<T: serde::Serialize>(ui: &SozoUi, value: &T, raw: bool) -> Result<()> {
    let json_str = serde_json::to_string_pretty(value)?;
    if raw {
        ui.print(json_str);
    } else {
        // Use colored JSON output
        let colored = json_str.to_colored_json_auto()?;
        ui.print(colored);
    }
    Ok(())
}
