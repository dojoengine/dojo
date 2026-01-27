use anyhow::Result;
use clap::Args;
use sozo_ui::SozoUi;
use starknet::core::types::requests::{
    GetTransactionByHashRequest, GetTransactionReceiptRequest, GetTransactionStatusRequest,
};
use starknet::core::types::Felt;
use starknet::providers::{Provider, ProviderRequestData, ProviderResponseData};
use tracing::trace;

use super::{parse_felt, print_json, OutputOptions};
use crate::commands::options::starknet::StarknetOptions;

#[derive(Debug, Args)]
#[command(about = "Get transaction by hash")]
pub struct TransactionArgs {
    #[arg(help = "Transaction hash(es) - supports multiple for batching", required = true)]
    pub transaction_hashes: Vec<String>,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl TransactionArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let hashes: Vec<Felt> = self
            .transaction_hashes
            .iter()
            .map(|h| parse_felt(h))
            .collect::<Result<Vec<_>>>()?;

        let (provider, _) = self.starknet.provider(None)?;

        if hashes.len() == 1 {
            // Single request (existing behavior)
            let tx = provider.get_transaction_by_hash(hashes[0]).await?;
            print_json(ui, &tx, self.output.raw)
        } else {
            // Batch request
            let requests: Vec<ProviderRequestData> = hashes
                .iter()
                .map(|h| {
                    ProviderRequestData::GetTransactionByHash(GetTransactionByHashRequest {
                        transaction_hash: *h,
                    })
                })
                .collect();

            let responses = provider.batch_requests(&requests).await?;

            // Extract the transaction data from responses
            let txs: Vec<_> = responses
                .into_iter()
                .map(|r| match r {
                    ProviderResponseData::GetTransactionByHash(tx) => tx,
                    _ => panic!("Unexpected response type"),
                })
                .collect();

            print_json(ui, &txs, self.output.raw)
        }
    }
}

#[derive(Debug, Args)]
#[command(about = "Get transaction receipt")]
pub struct ReceiptArgs {
    #[arg(help = "Transaction hash(es) - supports multiple for batching", required = true)]
    pub transaction_hashes: Vec<String>,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl ReceiptArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let hashes: Vec<Felt> = self
            .transaction_hashes
            .iter()
            .map(|h| parse_felt(h))
            .collect::<Result<Vec<_>>>()?;

        let (provider, _) = self.starknet.provider(None)?;

        if hashes.len() == 1 {
            // Single request (existing behavior)
            let receipt = provider.get_transaction_receipt(hashes[0]).await?;
            print_json(ui, &receipt, self.output.raw)
        } else {
            // Batch request
            let requests: Vec<ProviderRequestData> = hashes
                .iter()
                .map(|h| {
                    ProviderRequestData::GetTransactionReceipt(GetTransactionReceiptRequest {
                        transaction_hash: *h,
                    })
                })
                .collect();

            let responses = provider.batch_requests(&requests).await?;

            // Extract the receipt data from responses
            let receipts: Vec<_> = responses
                .into_iter()
                .map(|r| match r {
                    ProviderResponseData::GetTransactionReceipt(receipt) => receipt,
                    _ => panic!("Unexpected response type"),
                })
                .collect();

            print_json(ui, &receipts, self.output.raw)
        }
    }
}

#[derive(Debug, Args)]
#[command(about = "Get transaction status")]
pub struct StatusArgs {
    #[arg(help = "Transaction hash(es) - supports multiple for batching", required = true)]
    pub transaction_hashes: Vec<String>,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub output: OutputOptions,
}

impl StatusArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let hashes: Vec<Felt> = self
            .transaction_hashes
            .iter()
            .map(|h| parse_felt(h))
            .collect::<Result<Vec<_>>>()?;

        let (provider, _) = self.starknet.provider(None)?;

        if hashes.len() == 1 {
            // Single request (existing behavior)
            let status = provider.get_transaction_status(hashes[0]).await?;
            print_json(ui, &status, self.output.raw)
        } else {
            // Batch request
            let requests: Vec<ProviderRequestData> = hashes
                .iter()
                .map(|h| {
                    ProviderRequestData::GetTransactionStatus(GetTransactionStatusRequest {
                        transaction_hash: *h,
                    })
                })
                .collect();

            let responses = provider.batch_requests(&requests).await?;

            // Extract the status data from responses
            let statuses: Vec<_> = responses
                .into_iter()
                .map(|r| match r {
                    ProviderResponseData::GetTransactionStatus(status) => status,
                    _ => panic!("Unexpected response type"),
                })
                .collect();

            print_json(ui, &statuses, self.output.raw)
        }
    }
}
