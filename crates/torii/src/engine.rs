use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use starknet::core::types::{
    BlockId, BlockWithTxs, Event, InvokeTransaction, MaybePendingBlockWithTxs,
    MaybePendingTransactionReceipt, StarknetError, Transaction, TransactionReceipt,
};
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use starknet::providers::{Provider, ProviderError};
use tokio::time::sleep;
use tracing::error;

use crate::processors::{BlockProcessor, EventProcessor, TransactionProcessor};
use crate::state::State;

pub struct Processors<S: State, T: JsonRpcTransport + Sync + Send> {
    block: Vec<Arc<dyn BlockProcessor<S, T>>>,
    transaction: Vec<Arc<dyn TransactionProcessor<S, T>>>,
    event: Vec<Arc<dyn EventProcessor<S, T>>>,
}

impl<S: State, T: JsonRpcTransport + Sync + Send> Default for Processors<S, T> {
    fn default() -> Self {
        Self { block: vec![], transaction: vec![], event: vec![] }
    }
}

pub struct Engine<'a, S: State, T: JsonRpcTransport + Sync + Send> {
    storage: &'a S,
    provider: &'a JsonRpcClient<T>,
    processors: Processors<S, T>,
}

impl<'a, S: State, T: JsonRpcTransport + Sync + Send> Engine<'a, S, T> {
    pub fn new(
        storage: &'a S,
        provider: &'a JsonRpcClient<T>,
        processors: Processors<S, T>,
    ) -> Self {
        Self { storage, provider, processors }
    }

    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        let mut current_block_number = self.storage.head().await?;

        loop {
            sleep(Duration::from_secs(1)).await;

            let block_with_txs =
                match self.provider.get_block_with_txs(BlockId::Number(current_block_number)).await
                {
                    Ok(block_with_txs) => block_with_txs,
                    Err(e) => {
                        if let ProviderError::StarknetError(StarknetError::BlockNotFound) = e {
                            continue;
                        }

                        error!("getting  block: {}", e);
                        continue;
                    }
                };

            let block_with_txs = match block_with_txs {
                MaybePendingBlockWithTxs::Block(block_with_txs) => block_with_txs,
                _ => continue,
            };

            process_block(self.storage, self.provider, &self.processors.block, &block_with_txs)
                .await?;

            for transaction in block_with_txs.transactions {
                let invoke_transaction = match &transaction {
                    Transaction::Invoke(invoke_transaction) => invoke_transaction,
                    _ => continue,
                };

                let invoke_transaction = match invoke_transaction {
                    InvokeTransaction::V1(invoke_transaction) => invoke_transaction,
                    _ => continue,
                };

                let receipt = match self
                    .provider
                    .get_transaction_receipt(invoke_transaction.transaction_hash)
                    .await
                {
                    Ok(receipt) => receipt,
                    _ => continue,
                };

                let receipt = match receipt {
                    MaybePendingTransactionReceipt::Receipt(receipt) => receipt,
                    _ => continue,
                };

                process_transaction(
                    self.storage,
                    self.provider,
                    &self.processors.transaction,
                    &receipt.clone(),
                )
                .await?;

                if let TransactionReceipt::Invoke(invoke_receipt) = receipt.clone() {
                    for event in &invoke_receipt.events {
                        process_event(
                            self.storage,
                            self.provider,
                            &self.processors.event,
                            &receipt,
                            event,
                        )
                        .await?;
                    }
                }
            }

            current_block_number += 1;
        }
    }
}

async fn process_block<S: State, T: starknet::providers::jsonrpc::JsonRpcTransport>(
    storage: &S,
    provider: &JsonRpcClient<T>,
    processors: &[Arc<dyn BlockProcessor<S, T>>],
    block: &BlockWithTxs,
) -> Result<(), Box<dyn Error>> {
    for processor in processors {
        processor.process(storage, provider, block).await?;
    }
    Ok(())
}

async fn process_transaction<S: State, T: starknet::providers::jsonrpc::JsonRpcTransport>(
    storage: &S,
    provider: &JsonRpcClient<T>,
    processors: &[Arc<dyn TransactionProcessor<S, T>>],
    receipt: &TransactionReceipt,
) -> Result<(), Box<dyn Error>> {
    for processor in processors {
        processor.process(storage, provider, receipt).await?;
    }

    Ok(())
}

async fn process_event<S: State, T: starknet::providers::jsonrpc::JsonRpcTransport>(
    storage: &S,
    provider: &JsonRpcClient<T>,
    processors: &[Arc<dyn EventProcessor<S, T>>],
    _receipt: &TransactionReceipt,
    event: &Event,
) -> Result<(), Box<dyn Error>> {
    for processor in processors {
        processor.process(storage, provider, event).await?;
    }

    Ok(())
}
