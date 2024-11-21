use std::time::Duration;

use katana_primitives::block::{BlockNumber, SealedBlockWithStatus};
use katana_primitives::state::{StateUpdates, StateUpdatesWithClasses};
use katana_provider::traits::block::BlockWriter;
use starknet::providers::sequencer::models::{BlockId, StateUpdateWithBlock};
use starknet::providers::{ProviderError, SequencerGatewayProvider};

use super::{Stage, StageExecutionInput, StageResult};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Gateway(#[from] ProviderError),
}

#[derive(Debug)]
pub struct Blocks<P> {
    provider: P,
    feeder_gateway: SequencerGatewayProvider,
}

impl<P> Blocks<P> {
    pub fn new(provider: P, feeder_gateway: SequencerGatewayProvider) -> Self {
        Self { provider, feeder_gateway }
    }
}

impl<P: BlockWriter> Blocks<P> {
    #[allow(deprecated)]
    async fn fetch_block(&self, block: BlockNumber) -> Result<StateUpdateWithBlock, Error> {
        let res = self.feeder_gateway.get_state_update_with_block(BlockId::Number(block)).await?;
        Ok(res)
    }
}

#[async_trait::async_trait]
impl<P: BlockWriter> Stage for Blocks<P> {
    fn id(&self) -> &'static str {
        "Blocks"
    }

    async fn execute(&mut self, input: &StageExecutionInput) -> StageResult {
        let mut current_block = input.from;

        loop {
            let data = self.fetch_block(current_block).await?;
            let StateUpdateWithBlock { state_update, block: fgw_block } = data;

            let block = SealedBlockWithStatus::from(fgw_block);
            let su = StateUpdates::from(state_update);
            let su = StateUpdatesWithClasses { state_updates: su, ..Default::default() };

            let _ = self.provider.insert_block_with_states_and_receipts(
                block,
                su,
                Vec::new(),
                Vec::new(),
            );

            tokio::time::sleep(Duration::from_secs(1)).await;

            if current_block == input.to {
                break;
            } else {
                current_block += 1;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use katana_provider::test_utils::test_provider;
    use katana_provider::traits::block::BlockNumberProvider;
    use starknet::providers::SequencerGatewayProvider;

    use super::Blocks;
    use crate::stage::{Stage, StageExecutionInput};

    #[tokio::test]
    async fn fetch_blocks() {
        let from_block = 308919;
        let to_block = from_block + 2;

        let provider = test_provider();
        let feeder_gateway = SequencerGatewayProvider::starknet_alpha_sepolia();

        let mut stage = Blocks::new(&provider, feeder_gateway);

        let input = StageExecutionInput { from: from_block, to: to_block };
        let _ = stage.execute(&input).await.expect("failed to execute stage");

        // check provider storage
        let block_number = provider.latest_number().expect("failed to get latest block number");
        assert_eq!(block_number, to_block);
    }
}
