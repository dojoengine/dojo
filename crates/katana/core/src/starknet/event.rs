use starknet_api::block::{BlockHash, BlockNumber};
use starknet_api::transaction::{Event, TransactionHash};

pub struct EmittedEvent {
    pub inner: Event,
    pub block_hash: BlockHash,
    pub block_number: BlockNumber,
    pub transaction_hash: TransactionHash,
}
