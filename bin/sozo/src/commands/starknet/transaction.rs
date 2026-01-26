use anyhow::Result;
use clap::Args;
use sozo_ui::SozoUi;
use starknet::providers::Provider;
use tracing::trace;

use super::{parse_felt, print_json, OutputOptions};
use crate::commands::options::starknet::StarknetOptions;

#[derive(Debug, Args)]
#[command(about = "Get transaction by hash")]
pub struct TransactionArgs {
    #[arg(help = "The transaction hash (hex or decimal)")]
    pub transaction_hash: String,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl TransactionArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let tx_hash = parse_felt(&self.transaction_hash)?;
        let (provider, _) = self.starknet.provider(None)?;

        let tx = provider.get_transaction_by_hash(tx_hash).await?;

        print_json(ui, &tx, self.output.raw)
    }
}

#[derive(Debug, Args)]
#[command(about = "Get transaction receipt")]
pub struct ReceiptArgs {
    #[arg(help = "The transaction hash (hex or decimal)")]
    pub transaction_hash: String,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl ReceiptArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let tx_hash = parse_felt(&self.transaction_hash)?;
        let (provider, _) = self.starknet.provider(None)?;

        let receipt = provider.get_transaction_receipt(tx_hash).await?;

        print_json(ui, &receipt, self.output.raw)
    }
}

#[derive(Debug, Args)]
#[command(about = "Get transaction status")]
pub struct StatusArgs {
    #[arg(help = "The transaction hash (hex or decimal)")]
    pub transaction_hash: String,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl StatusArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let tx_hash = parse_felt(&self.transaction_hash)?;
        let (provider, _) = self.starknet.provider(None)?;

        let status = provider.get_transaction_status(tx_hash).await?;

        print_json(ui, &status, self.output.raw)
    }
}
