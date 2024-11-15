use std::time::Duration;

use katana_primitives::block::{BlockNumber, SealedBlockWithStatus};
use katana_primitives::state::{StateUpdates, StateUpdatesWithDeclaredClasses};
use katana_provider::traits::block::BlockWriter;
use katana_provider::traits::stage::StageCheckpointProvider;
use starknet::providers::sequencer::models::{BlockId, StateUpdateWithBlock};
use starknet::providers::{ProviderError, SequencerGatewayProvider};

// Blocks -> Tx traces -> Classes (repeat)
use super::{Stage, StageExecutionInput, StageExecutionOutput, StageResult};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Gateway(#[from] ProviderError),
}

pub struct Blocks<P> {
    provider: P,
    feeder_gateway: SequencerGatewayProvider,
}

impl<P> Blocks<P>
where
    P: BlockWriter + StageCheckpointProvider,
{
    async fn fetch_blocks(&self, block: BlockNumber) -> Result<StateUpdateWithBlock, Error> {
        #[allow(deprecated)]
        let res = self.feeder_gateway.get_state_update_with_block(BlockId::Number(block)).await?;
        Ok(res)
    }
}

#[async_trait::async_trait]
impl<P> Stage for Blocks<P>
where
    P: BlockWriter + StageCheckpointProvider,
{
    fn id(&self) -> &'static str {
        "Blocks"
    }

    async fn execute(&mut self, input: &StageExecutionInput) -> StageResult {
        // Get checkpoint from storage
        let checkpoint = self.provider.checkpoint(self.id())?.unwrap_or_default();
        let mut current_block = checkpoint;

        loop {
            let data = self.fetch_blocks(current_block).await?;
            let StateUpdateWithBlock { state_update, block: fgw_block } = data;

            let block = SealedBlockWithStatus::from(fgw_block);
            let su = StateUpdates::from(state_update);
            let su = StateUpdatesWithDeclaredClasses { state_updates: su, ..Default::default() };

            let _ = self.provider.insert_block_with_states_and_receipts(
                block,
                su,
                Vec::new(),
                Vec::new(),
            );

            tokio::time::sleep(Duration::from_secs(1)).await;

            if current_block == input.to {
                break;
            }

            current_block += 1;
        }

        Ok(StageExecutionOutput { last_block_processed: current_block })
    }
}

#[cfg(test)]
mod tests {
    use starknet::providers::SequencerGatewayProvider;

    // #[tokio::test]
    // async fn fetch_blocks() {
    //     let from_block = 308919;
    //     let fgw = SequencerGatewayProvider::starknet_alpha_sepolia();
    //     let client = super::Blocks { feeder_gateway: fgw, from_block };

    //     let blocks = client.fetch_blocks(from_block + 3).await.unwrap();

    //     // assert_eq!(blocks.len(), 4);
    //     // println!("{:?}", blocks[3]);
    // }
}
