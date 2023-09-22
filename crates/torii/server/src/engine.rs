use std::error::Error;
use std::time::Duration;

use starknet::core::types::{
    BlockId, BlockTag, BlockWithTxs, Event, InvokeTransaction, InvokeTransactionReceipt,
    MaybePendingBlockWithTxs, MaybePendingTransactionReceipt, Transaction, TransactionReceipt,
};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;
use starknet_crypto::FieldElement;
use tokio::time::sleep;
use torii_client::contract::world::WorldContractReader;
use torii_core::processors::{BlockProcessor, EventProcessor, TransactionProcessor};
use torii_core::sql::{Executable, Sql};
use tracing::{error, info, warn};

pub struct Processors<P: Provider + Sync + Send> {
    pub block: Vec<Box<dyn BlockProcessor<P>>>,
    pub transaction: Vec<Box<dyn TransactionProcessor<P>>>,
    pub event: Vec<Box<dyn EventProcessor<P>>>,
}

impl<P: Provider + Sync + Send> Default for Processors<P> {
    fn default() -> Self {
        Self { block: vec![], transaction: vec![], event: vec![] }
    }
}

#[derive(Debug)]
pub struct EngineConfig {
    pub block_time: Duration,
    pub world_address: FieldElement,
    pub start_block: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            block_time: Duration::from_secs(1),
            world_address: FieldElement::ZERO,
            start_block: 0,
        }
    }
}

pub struct Engine<'a, P: Provider + Sync + Send> {
    world: &'a WorldContractReader<'a, P>,
    db: &'a Sql,
    provider: &'a P,
    processors: Processors<P>,
    config: EngineConfig,
}

impl<'a, P: Provider + Sync + Send> Engine<'a, P> {
    pub fn new(
        world: &'a WorldContractReader<'a, P>,
        db: &'a Sql,
        provider: &'a P,
        processors: Processors<P>,
        config: EngineConfig,
    ) -> Self {
        Self { world, db, provider, processors, config }
    }

    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        let db_head = self.db.head().await?;

        let mut current_block_number = match db_head {
            0 => self.config.start_block,
            _ => {
                if self.config.start_block != 0 {
                    warn!("start block ignored, stored head exists and will be used instead");
                }
                db_head
            }
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

                self.db.set_head(current_block_number).await?;
                self.db.execute().await?;
                current_block_number += 1;
            }
        }
    }

    async fn process(&self, block: MaybePendingBlockWithTxs) -> Result<(), Box<dyn Error>> {
        let block: BlockWithTxs = match block {
            MaybePendingBlockWithTxs::Block(block) => block,
            _ => return Ok(()),
        };

        process_block(self.db, self.provider, &self.processors.block, &block).await?;

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

            if let TransactionReceipt::Invoke(invoke_receipt) = receipt.clone() {
                for (event_idx, event) in invoke_receipt.events.iter().enumerate() {
                    if event.from_address != self.config.world_address {
                        continue;
                    }

                    process_event(
                        self.world,
                        self.db,
                        self.provider,
                        &self.processors.event,
                        &block,
                        &invoke_receipt,
                        event,
                        event_idx,
                    )
                    .await?;
                }
            }

            process_transaction(
                self.db,
                self.provider,
                &self.processors.transaction,
                &block,
                &receipt.clone(),
            )
            .await?;
        }

        info!("processed block: {}", block.block_number);

        Ok(())
    }
}

async fn process_block<P: Provider + Sync>(
    db: &Sql,
    provider: &P,
    processors: &[Box<dyn BlockProcessor<P>>],
    block: &BlockWithTxs,
) -> Result<(), Box<dyn Error>> {
    for processor in processors {
        processor.process(db, provider, block).await?;
    }
    Ok(())
}

async fn process_transaction<P: Provider + Sync>(
    db: &Sql,
    provider: &P,
    processors: &[Box<dyn TransactionProcessor<P>>],
    block: &BlockWithTxs,
    receipt: &TransactionReceipt,
) -> Result<(), Box<dyn Error>> {
    for processor in processors {
        processor.process(db, provider, block, receipt).await?
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn process_event<P: Provider + Sync>(
    world: &WorldContractReader<'_, P>,
    db: &Sql,
    provider: &P,
    processors: &[Box<dyn EventProcessor<P>>],
    block: &BlockWithTxs,
    invoke_receipt: &InvokeTransactionReceipt,
    event: &Event,
    event_idx: usize,
) -> Result<(), Box<dyn Error>> {
    db.store_event(event, event_idx, invoke_receipt.transaction_hash).await?;

    for processor in processors {
        if get_selector_from_name(&processor.event_key())? == event.keys[0] {
            processor.process(world, db, provider, block, invoke_receipt, event).await?;
        }
    }

    Ok(())
}
