use anyhow::Result;
use blockifier::block_context::BlockContext;
use katana_primitives::block::{
    Block, FinalityStatus, Header, PartialHeader, SealedBlock, SealedBlockWithStatus,
};
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_provider::traits::block::{BlockProvider, BlockWriter};
use katana_provider::traits::contract::ContractClassWriter;
use katana_provider::traits::state::{StateFactoryProvider, StateRootProvider, StateWriter};
use katana_provider::traits::state_update::StateUpdateProvider;
use katana_provider::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider, TransactionsProviderExt,
};
use katana_provider::BlockchainProvider;

use crate::constants::SEQUENCER_ADDRESS;
use crate::utils::get_genesis_states_for_testing;

pub trait Database:
    BlockProvider
    + BlockWriter
    + TransactionProvider
    + TransactionStatusProvider
    + TransactionsProviderExt
    + ReceiptProvider
    + StateUpdateProvider
    + StateRootProvider
    + StateWriter
    + ContractClassWriter
    + StateFactoryProvider
    + 'static
    + Send
    + Sync
{
}

impl<T> Database for T where
    T: BlockProvider
        + BlockWriter
        + TransactionProvider
        + TransactionStatusProvider
        + TransactionsProviderExt
        + ReceiptProvider
        + StateUpdateProvider
        + StateRootProvider
        + StateWriter
        + ContractClassWriter
        + StateFactoryProvider
        + 'static
        + Send
        + Sync
{
}

pub struct Blockchain {
    inner: BlockchainProvider<Box<dyn Database>>,
}

impl Blockchain {
    pub fn new(provider: impl Database) -> Self {
        Self { inner: BlockchainProvider::new(Box::new(provider)) }
    }

    pub fn provider(&self) -> &BlockchainProvider<Box<dyn Database>> {
        &self.inner
    }

    pub fn new_with_genesis(provider: impl Database, block_context: &BlockContext) -> Result<Self> {
        let header = PartialHeader {
            gas_price: block_context.gas_price,
            number: 0,
            parent_hash: 0u8.into(),
            timestamp: block_context.block_timestamp.0,
            sequencer_address: *SEQUENCER_ADDRESS,
        };

        let block = Block { header: Header::new(header, 0u8.into()), body: vec![] }.seal();
        Self::new_with_block_and_state(provider, block, get_genesis_states_for_testing())
    }

    fn new_with_block_and_state(
        provider: impl Database,
        block: SealedBlock,
        states: StateUpdatesWithDeclaredClasses,
    ) -> Result<Self> {
        let block = SealedBlockWithStatus { block, status: FinalityStatus::AcceptedOnL1 };
        BlockWriter::insert_block_with_states_and_receipts(&provider, block, states, vec![])?;
        Ok(Self::new(provider))
    }
}
