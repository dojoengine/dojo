use std::error::Error;
use std::time::Duration;

use starknet::core::types::{
    BlockId, BlockTag, BlockWithTxs, Event, InvokeTransaction, MaybePendingBlockWithTxs,
    MaybePendingTransactionReceipt, Transaction, TransactionReceipt,
};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use starknet::providers::Provider;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::processors::{BlockProcessor, EventProcessor, TransactionProcessor};
use crate::state::sql::Executable;
use crate::state::State;

pub struct Processors<S: State, T: JsonRpcTransport + Sync + Send> {
    pub block: Vec<Box<dyn BlockProcessor<S, T>>>,
    pub transaction: Vec<Box<dyn TransactionProcessor<S, T>>>,
    pub event: Vec<Box<dyn EventProcessor<S, T>>>,
}

impl<S: State, T: JsonRpcTransport + Sync + Send> Default for Processors<S, T> {
    fn default() -> Self {
        Self { block: vec![], transaction: vec![], event: vec![] }
    }
}

#[derive(Debug)]
pub struct EngineConfig {
    pub block_time: Duration,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self { block_time: Duration::from_secs(1) }
    }
}

pub struct Engine<'a, S: State + Executable, T: JsonRpcTransport + Sync + Send> {
    storage: &'a S,
    provider: &'a JsonRpcClient<T>,
    processors: Processors<S, T>,
    start_block: Option<u64>,
    config: EngineConfig,
}

impl<'a, S: State + Executable, T: JsonRpcTransport + Sync + Send> Engine<'a, S, T> {
    pub fn new(
        storage: &'a S,
        provider: &'a JsonRpcClient<T>,
        processors: Processors<S, T>,
        start_block: Option<u64>,
        config: EngineConfig,
    ) -> Self {
        Self { storage, provider, processors, start_block, config }
    }

    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        let storage_head = self.storage.head().await?;

        let mut current_block_number = match (storage_head, self.start_block) {
            (0, Some(start_block)) => start_block,
            (_, Some(_)) => {
                warn!("start block ignored, stored head exists and will be used instead");
                storage_head
            }
            (_, None) => storage_head,
        };

        loop {
            sleep(self.config.block_time).await;

            let latest_block_with_txs =
                match self.provider.get_block_with_txs(BlockId::Tag(BlockTag::Latest)).await {
                    Ok(block_with_txs) => block_with_txs,
                    Err(e) => {
                        error!("getting  block: {}", e);
                        continue;
                    }
                };

            let latest_block_number = match latest_block_with_txs {
                MaybePendingBlockWithTxs::Block(latest_block_with_txs) => {
                    latest_block_with_txs.block_number
                }
                _ => continue,
            };

            // Process all blocks from current to latest.
            while current_block_number <= latest_block_number {
                let block_with_txs = match self
                    .provider
                    .get_block_with_txs(BlockId::Number(current_block_number))
                    .await
                {
                    Ok(block_with_txs) => block_with_txs,
                    Err(e) => {
                        error!("getting block: {}", e);
                        continue;
                    }
                };

                self.process(block_with_txs).await?;

                self.storage.set_head(current_block_number).await?;
                self.storage.execute().await?;
                current_block_number += 1;
            }
        }
    }

    async fn process(&self, block: MaybePendingBlockWithTxs) -> Result<(), Box<dyn Error>> {
        let block: BlockWithTxs = match block {
            MaybePendingBlockWithTxs::Block(block) => block,
            _ => return Ok(()),
        };

        process_block(self.storage, self.provider, &self.processors.block, &block).await?;

        for transaction in block.clone().transactions {
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
                &block,
                &receipt.clone(),
            )
            .await?;

            if let TransactionReceipt::Invoke(invoke_receipt) = receipt.clone() {
                for event in &invoke_receipt.events {
                    process_event(
                        self.storage,
                        self.provider,
                        &self.processors.event,
                        &block,
                        &receipt,
                        event,
                    )
                    .await?;
                }
            }
        }

        info!("processed block: {}", block.block_number);

        Ok(())
    }
}

async fn process_block<S: State, T: starknet::providers::jsonrpc::JsonRpcTransport>(
    storage: &S,
    provider: &JsonRpcClient<T>,
    processors: &[Box<dyn BlockProcessor<S, T>>],
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
    processors: &[Box<dyn TransactionProcessor<S, T>>],
    block: &BlockWithTxs,
    receipt: &TransactionReceipt,
) -> Result<(), Box<dyn Error>> {
    for processor in processors {
        processor.process(storage, provider, block, receipt).await?;
    }

    Ok(())
}

async fn process_event<S: State, T: starknet::providers::jsonrpc::JsonRpcTransport>(
    storage: &S,
    provider: &JsonRpcClient<T>,
    processors: &[Box<dyn EventProcessor<S, T>>],
    block: &BlockWithTxs,
    receipt: &TransactionReceipt,
    event: &Event,
) -> Result<(), Box<dyn Error>> {
    for processor in processors {
        if get_selector_from_name(&processor.event_key())? == event.keys[0] {
            processor.process(storage, provider, block, receipt, event).await?;
        }
    }

    Ok(())
}
