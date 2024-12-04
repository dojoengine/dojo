use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use backon::{ExponentialBuilder, Retryable};
use katana_feeder_gateway::client;
use katana_feeder_gateway::client::SequencerGateway;
use katana_feeder_gateway::types::StateUpdateWithBlock;
use katana_primitives::block::{
    BlockIdOrTag, BlockNumber, FinalityStatus, GasPrices, Header, SealedBlock,
    SealedBlockWithStatus,
};
use katana_primitives::fee::{PriceUnit, TxFeeInfo};
use katana_primitives::receipt::{
    DeclareTxReceipt, DeployAccountTxReceipt, InvokeTxReceipt, L1HandlerTxReceipt, Receipt,
};
use katana_primitives::state::{StateUpdates, StateUpdatesWithClasses};
use katana_primitives::transaction::{Tx, TxWithHash};
use katana_primitives::Felt;
use katana_provider::traits::block::BlockWriter;
use num_traits::ToPrimitive;
use starknet::core::types::ResourcePrice;
use starknet::providers::sequencer::models::BlockStatus;
use tracing::{debug, error, warn};

use super::{Stage, StageExecutionInput, StageResult};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Gateway(#[from] client::Error),
}

#[derive(Debug)]
pub struct Blocks<P> {
    provider: P,
    downloader: Downloader,
}

impl<P> Blocks<P> {
    pub fn new(provider: P, feeder_gateway: SequencerGateway, download_batch_size: usize) -> Self {
        let downloader = Downloader::new(feeder_gateway, download_batch_size);
        Self { provider, downloader }
    }
}

#[async_trait::async_trait]
impl<P: BlockWriter> Stage for Blocks<P> {
    fn id(&self) -> &'static str {
        "Blocks"
    }

    async fn execute(&mut self, input: &StageExecutionInput) -> StageResult {
        // Download all blocks from the provided range
        let blocks = self.downloader.download_blocks(input.from, input.to).await?;

        if !blocks.is_empty() {
            debug!(target: "stage", id = %self.id(), total = %blocks.len(), "Storing blocks to storage.");

            // Store blocks to storage
            for block in blocks {
                let (block, receipts, state_updates) = extract_block_data(block)?;

                self.provider.insert_block_with_states_and_receipts(
                    block,
                    state_updates,
                    receipts,
                    Vec::new(),
                )?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Downloader {
    batch_size: usize,
    client: Arc<SequencerGateway>,
}

impl Downloader {
    fn new(client: SequencerGateway, batch_size: usize) -> Self {
        Self { client: Arc::new(client), batch_size }
    }

    /// Fetch blocks in the range [from, to] in batches of `batch_size`.
    async fn download_blocks(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<StateUpdateWithBlock>, Error> {
        debug!(target: "pipeline", %from, %to, "Downloading blocks.");
        let mut blocks = Vec::with_capacity(to.saturating_sub(from) as usize);

        for batch_start in (from..=to).step_by(self.batch_size) {
            let batch_end = (batch_start + self.batch_size as u64 - 1).min(to);
            let batch = self.fetch_blocks_with_retry(batch_start, batch_end).await?;
            blocks.extend(batch);
        }

        Ok(blocks)
    }

    /// Fetch blocks with the given block number with retry mechanism at a batch level.
    async fn fetch_blocks_with_retry(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<StateUpdateWithBlock>, Error> {
        let request = || async move { self.clone().fetch_blocks(from, to).await };

        // Retry only when being rate limited
        let backoff = ExponentialBuilder::default().with_min_delay(Duration::from_secs(9));
        let result = request
            .retry(backoff)
            .when(|error| matches!(error, Error::Gateway(client::Error::RateLimited)))
            .notify(|error, _| {
                warn!(target: "pipeline", %from, %to, %error, "Retrying block download.");
            })
            .await?;

        Ok(result)
    }

    async fn fetch_blocks(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<StateUpdateWithBlock>, Error> {
        let total = to.saturating_sub(from) as usize;
        let mut requests = Vec::with_capacity(total);

        for i in from..=to {
            requests.push(self.fetch_block(i));
        }

        let results = futures::future::join_all(requests).await;
        results.into_iter().collect()
    }

    /// Fetch a single block with the given block number.
    async fn fetch_block(&self, block: BlockNumber) -> Result<StateUpdateWithBlock, Error> {
        let block = self
            .client
            .get_state_update_with_block(BlockIdOrTag::Number(block))
            .await
            .inspect_err(|error| {
                if !error.is_rate_limited() {
                    error!(target: "pipeline", %error, %block, "Failed to fetch block.")
                }
            })?;
        Ok(block)
    }
}

fn extract_block_data(
    data: StateUpdateWithBlock,
) -> Result<(SealedBlockWithStatus, Vec<Receipt>, StateUpdatesWithClasses)> {
    fn to_gas_prices(prices: ResourcePrice) -> GasPrices {
        GasPrices {
            eth: prices.price_in_fri.to_u128().expect("valid u128"),
            strk: prices.price_in_fri.to_u128().expect("valid u128"),
        }
    }

    let status = match data.block.status {
        BlockStatus::AcceptedOnL2 => FinalityStatus::AcceptedOnL2,
        BlockStatus::AcceptedOnL1 => FinalityStatus::AcceptedOnL1,
        status => panic!("unsupported block status: {status:?}"),
    };

    let transactions = data
        .block
        .transactions
        .into_iter()
        .map(|tx| tx.try_into())
        .collect::<Result<Vec<TxWithHash>, _>>()?;

    let receipts = data
        .block
        .transaction_receipts
        .into_iter()
        .zip(transactions.iter())
        .map(|(receipt, tx)| {
            let events = receipt.events;
            let revert_error = receipt.revert_error;
            let messages_sent = receipt.l2_to_l1_messages;
            let overall_fee = receipt.actual_fee.to_u128().expect("valid u128");

            let unit = if tx.transaction.version() >= Felt::THREE {
                PriceUnit::Fri
            } else {
                PriceUnit::Wei
            };

            let fee = TxFeeInfo {
                unit,
                overall_fee,
                gas_price: Default::default(),
                gas_consumed: Default::default(),
            };

            match tx.transaction {
                Tx::Invoke(_) => Receipt::Invoke(InvokeTxReceipt {
                    fee,
                    events,
                    revert_error,
                    messages_sent,
                    execution_resources: Default::default(),
                }),
                Tx::Declare(_) => Receipt::Declare(DeclareTxReceipt {
                    fee,
                    events,
                    revert_error,
                    messages_sent,
                    execution_resources: Default::default(),
                }),
                Tx::L1Handler(_) => Receipt::L1Handler(L1HandlerTxReceipt {
                    fee,
                    events,
                    messages_sent,
                    revert_error,
                    message_hash: Default::default(),
                    execution_resources: Default::default(),
                }),
                Tx::DeployAccount(_) => Receipt::DeployAccount(DeployAccountTxReceipt {
                    fee,
                    events,
                    revert_error,
                    messages_sent,
                    contract_address: Default::default(),
                    execution_resources: Default::default(),
                }),
                Tx::Deploy(_) => unreachable!("Deploy transactions are not supported"),
            }
        })
        .collect::<Vec<Receipt>>();

    let transaction_count = transactions.len() as u32;
    let block = SealedBlock {
        body: transactions,
        hash: data.block.block_hash.unwrap_or_default(),
        header: Header {
            transaction_count,
            timestamp: data.block.timestamp,
            l1_da_mode: data.block.l1_da_mode,
            events_count: Default::default(),
            parent_hash: data.block.parent_block_hash,
            state_diff_length: Default::default(),
            receipts_commitment: Default::default(),
            state_diff_commitment: Default::default(),
            number: data.block.block_number.unwrap_or_default(),
            l1_gas_prices: to_gas_prices(data.block.l1_gas_price),
            state_root: data.block.state_root.unwrap_or_default(),
            l1_data_gas_prices: to_gas_prices(data.block.l1_data_gas_price),
            protocol_version: data.block.starknet_version.unwrap_or_default(),
            events_commitment: data.block.event_commitment.unwrap_or_default(),
            sequencer_address: data.block.sequencer_address.unwrap_or_default(),
            transactions_commitment: data.block.transaction_commitment.unwrap_or_default(),
        },
    };

    let state_updates: StateUpdates = data.state_update.state_diff.into();
    let state_updates = StateUpdatesWithClasses { state_updates, ..Default::default() };

    Ok((SealedBlockWithStatus { block, status }, receipts, state_updates))
}

#[cfg(test)]
mod tests {
    use katana_feeder_gateway::client::SequencerGateway;
    use katana_provider::test_utils::test_provider;
    use katana_provider::traits::block::BlockNumberProvider;

    use super::Blocks;
    use crate::stage::{Stage, StageExecutionInput};

    #[tokio::test]
    async fn fetch_blocks() {
        let from_block = 308919;
        let to_block = from_block + 2;

        let provider = test_provider();
        let feeder_gateway = SequencerGateway::sn_sepolia();

        let mut stage = Blocks::new(&provider, feeder_gateway, 10);

        let input = StageExecutionInput { from: from_block, to: to_block };
        stage.execute(&input).await.expect("failed to execute stage");

        // check provider storage
        let block_number = provider.latest_number().expect("failed to get latest block number");
        assert_eq!(block_number, to_block);
    }
}
