use katana_primitives::block::BlockNumber;
use katana_primitives::contract::{Nonce, MessageHash};

use crate::ProviderResult;

pub const SEND_FROM_BLOCK_KEY: u64 = 1;
pub const GATHER_FROM_BLOCK_KEY: u64 = 2;
pub const GATHER_FROM_NONCE_KEY: u64 = 3;
pub const SEND_FROM_INDEX_KEY: u64 = 4;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait MessagingProvider: Send + Sync {
    /// Sets the send from block.
    fn set_send_from_block(&self, send_from_block: BlockNumber) -> ProviderResult<()>;
    /// Returns the send from block.
    fn get_send_from_block(&self) -> ProviderResult<Option<BlockNumber>>;
    /// Sets the gather from block.
    fn set_gather_from_block(&self, gather_from_block: BlockNumber) -> ProviderResult<()>;
    /// Returns the gather from block.
    fn get_gather_from_block(&self) -> ProviderResult<Option<BlockNumber>>;
    /// Sets the gather from nonce.
    fn set_gather_message_nonce(&self, nonce: Nonce) -> ProviderResult<()>;
    /// Returns the gather from nonce.
    fn get_gather_message_nonce(&self) -> ProviderResult<Option<Nonce>>;
    /// Sets the nonce by message_hash.
    fn set_nonce_from_message_hash(&self, message_hash: MessageHash, nonce: Nonce) -> ProviderResult<()>;
    /// Returns the nonce by message_hash.
    fn get_nonce_from_message_hash(&self, message_hash: MessageHash) -> ProviderResult<Option<Nonce>>;
    /// Sets the send from index.
    fn set_send_from_index(&self, index: u64) -> ProviderResult<()>;
    /// Returns the send from index.
    fn get_send_from_index(&self) -> ProviderResult<Option<u64>>;
}
