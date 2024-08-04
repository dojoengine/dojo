use anyhow::Error;
use async_trait::async_trait;
use cainome::cairo_serde::{CairoSerde, U256};
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{Event, TransactionReceiptWithBlockInfo};
use starknet::providers::Provider;

use super::EventProcessor;
use crate::sql::Sql;

// pub(crate) const LOG_TARGET: &str = "torii_core::processors::erc721_transfer";

#[derive(Default, Debug)]
pub struct Erc721TransferProcessor;

#[async_trait]
impl<P> EventProcessor<P> for Erc721TransferProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "Transfer".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        // ref: https://github.com/OpenZeppelin/cairo-contracts/blob/eabfa029b7b681d9e83bf171f723081b07891016/packages/token/src/erc721/erc721.cairo#L44-L53
        // key: [hash(Transfer), from, to, token_id.low, token_id.high]
        // data: []
        if event.keys.len() == 5 && event.data.len() == 0 {
            return true;
        }

        false
    }

    async fn process(
        &self,
        _world: &WorldContractReader<P>,
        _db: &mut Sql,
        _block_number: u64,
        _block_timestamp: u64,
        _transaction_receipt: &TransactionReceiptWithBlockInfo,
        _event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        let from = event.keys[1];
        let to = event.keys[2];

        let token_id = U256::cairo_deserialize(&event.keys, 3)?;
        println!("ERC721 Transfer from: {:?}, to: {:?}, value: {:?}", from, to, token_id);

        Ok(())
    }
}
