use std::convert::Infallible;
use std::time::Duration;

use katana_primitives::block::{BlockNumber, SealedBlock};
use katana_provider::traits::block::BlockWriter;
use starknet::providers::sequencer::models::StateUpdateWithBlock;
use starknet::providers::ProviderError;
use starknet::providers::{
    sequencer::models::{Block, BlockId},
    SequencerGatewayProvider, SequencerGatewayProviderError,
};

use super::{Stage, StageId, StageResult};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Gateway(#[from] ProviderError),
}

pub struct Blocks<P: BlockWriter> {
    feeder_gateway: SequencerGatewayProvider,
    from_block: BlockNumber,
    provider: P,
}

impl<P: BlockWriter> Blocks<P> {
    #[allow(deprecated)]
    async fn fetch_blocks(&self, block: BlockNumber) -> Result<StateUpdateWithBlock, Error> {
        let block_id = BlockId::Number(block);
        let res = self.feeder_gateway.get_state_update_with_block(block_id).await?;
        Ok(res)
    }
}

#[async_trait::async_trait]
impl<P: BlockWriter> Stage for Blocks<P> {
    fn id(&self) -> StageId {
        StageId::Blocks
    }

    async fn execute(&mut self) -> StageResult {
        let mut current_block = self.from_block;
        let mut blocks = Vec::new();

        loop {
            let data = self.fetch_blocks(current_block).await?;
            let StateUpdateWithBlock { state_update, block } = data;

            blocks.push(state_update_with_block);
            current_block += 1;

            if current_block > 10 {
                break;
            }

            let _ = self.provider.insert_block_with_states_and_receipts(
                block,
                Default::default(),
                Vec::new(),
                Vec::new(),
            );

            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        Ok(())
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

impl TryFrom<Block> for SealedBlock {
    type Error = Infallible;

    fn try_from(value: Block) -> Result<Self, Self::Error> {
        todo!()
    }
}
