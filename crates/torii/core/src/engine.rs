use std::error::Error;
use std::time::Duration;

use starknet::core::types::{
    BlockId, BlockWithTxs, Event, InvokeTransaction, InvokeTransactionReceipt,
    MaybePendingBlockWithTxs, MaybePendingTransactionReceipt, Transaction, TransactionReceipt,
};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;
use tokio::sync::mpsc::Sender as BoundedSender;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use torii_client::contract::world::WorldContractReader;
use tracing::{error, info, warn};

use crate::processors::{BlockProcessor, EventProcessor, TransactionProcessor};
use crate::sql::Sql;

pub struct Processors<P: Provider + Sync> {
    pub block: Vec<Box<dyn BlockProcessor<P>>>,
    pub transaction: Vec<Box<dyn TransactionProcessor<P>>>,
    pub event: Vec<Box<dyn EventProcessor<P>>>,
}

impl<P: Provider + Sync> Default for Processors<P> {
    fn default() -> Self {
        Self { block: vec![], event: vec![], transaction: vec![] }
    }
}

#[derive(Debug)]
pub struct EngineConfig {
    pub block_time: Duration,
    pub start_block: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self { block_time: Duration::from_secs(1), start_block: 0 }
    }
}

pub struct Engine<'a, P: Provider + Sync>
where
    P::Error: 'static,
{
    world: &'a WorldContractReader<'a, P>,
    db: &'a mut Sql,
    provider: &'a P,
    processors: Processors<P>,
    config: EngineConfig,
    block_sender: Option<BoundedSender<u64>>,
}

impl<'a, P: Provider + Sync> Engine<'a, P>
where
    P::Error: 'static,
{
    pub fn new(
        world: &'a WorldContractReader<'a, P>,
        db: &'a mut Sql,
        provider: &'a P,
        processors: Processors<P>,
        config: EngineConfig,
        block_sender: Option<BoundedSender<u64>>,
    ) -> Self {
        Self { world, db, provider, processors, config, block_sender }
    }

    pub async fn start(&mut self, cts: CancellationToken) -> Result<(), Box<dyn Error>> {
        let mut head = self.db.head().await?;
        if head == 0 {
            head = self.config.start_block;
        } else if self.config.start_block != 0 {
            warn!("start block ignored, stored head exists and will be used instead");
        }

        loop {
            if cts.is_cancelled() {
                break Ok(());
            }

            match self.sync_to_head(head).await {
                Ok(latest_block_number) => head = latest_block_number,
                Err(e) => {
                    error!("getting  block: {}", e);
                    continue;
                }
            };

            sleep(self.config.block_time).await;
        }
    }

    pub async fn sync_to_head(&mut self, from: u64) -> Result<u64, Box<dyn Error>> {
        let latest_block_number = self.provider.block_hash_and_number().await?.block_number;

        if from < latest_block_number {
            // if `from` == 0, then the block may or may not be processed yet.
            let from = if from == 0 { from } else { from + 1 };
            self.sync_range(from, latest_block_number).await?;
        };

        Ok(latest_block_number)
    }

    pub async fn sync_range(&mut self, mut from: u64, to: u64) -> Result<(), Box<dyn Error>> {
        // Process all blocks from current to latest.
        while from <= to {
            let block_with_txs = match self.provider.get_block_with_txs(BlockId::Number(from)).await
            {
                Ok(block_with_txs) => block_with_txs,
                Err(e) => {
                    error!("getting block: {}", e);
                    continue;
                }
            };

            // send the current block number
            if let Some(ref block_sender) = self.block_sender {
                block_sender.send(from).await.expect("failed to send block number to gRPC server");
            }

            self.process(block_with_txs).await?;

            self.db.set_head(from);
            self.db.execute().await?;
            from += 1;
        }

        Ok(())
    }

    async fn process(&mut self, block: MaybePendingBlockWithTxs) -> Result<(), Box<dyn Error>> {
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
                    if event.from_address != self.world.address {
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
    db: &mut Sql,
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
    db: &mut Sql,
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
    db: &mut Sql,
    provider: &P,
    processors: &[Box<dyn EventProcessor<P>>],
    block: &BlockWithTxs,
    invoke_receipt: &InvokeTransactionReceipt,
    event: &Event,
    event_idx: usize,
) -> Result<(), Box<dyn Error>> {
    db.store_event(event, event_idx, invoke_receipt.transaction_hash);

    for processor in processors {
        if get_selector_from_name(&processor.event_key())? == event.keys[0] {
            processor.process(world, db, provider, block, invoke_receipt, event).await?;
        }
    }

    Ok(())
}
