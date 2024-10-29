use katana_primitives::block::BlockNumber;
use katana_primitives::contract::{Nonce, MessageHash};
use katana_primitives::transaction::L1HandlerTx;
use crate::ProviderResult;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait MessagingCheckpointProvider: Send + Sync {
    /// Sets the outbound block.
    fn set_outbound_block(&self, outbound_block: BlockNumber) -> ProviderResult<()>;
    /// Returns the outbound block.
    fn get_outbound_block(&self) -> ProviderResult<Option<BlockNumber>>;

    /// Sets the inbound block.
    fn set_inbound_block(&self, inbound_block: BlockNumber) -> ProviderResult<()>;
    /// Returns the inbound block.
    fn get_inbound_block(&self) -> ProviderResult<Option<BlockNumber>>;
    /// Sets the inbound nonce.
    fn set_inbound_nonce(&self, nonce: Nonce) -> ProviderResult<()>;
    /// Returns the inbound nonce.
    fn get_inbound_nonce(&self) -> ProviderResult<Option<Nonce>>;
    /// Sets the nonce by message_hash.
    fn set_nonce_from_message_hash(&self, message_hash: MessageHash, nonce: Nonce) -> ProviderResult<()>;
    /// Returns the nonce by message_hash.
    fn get_nonce_from_message_hash(&self, message_hash: MessageHash) -> ProviderResult<Option<Nonce>>;
    /// Sets the outbound index.
    fn set_outbound_index(&self, index: u64) -> ProviderResult<()>;
    /// Returns the outbound index.
    fn get_outbound_index(&self) -> ProviderResult<Option<u64>>;
    /// Processes the nonce in the provided L1HandlerTx and updates the inbound nonce within the provider.
    fn process_message_nonce(&self, l1_tx: &L1HandlerTx) -> ProviderResult<()>;
}
