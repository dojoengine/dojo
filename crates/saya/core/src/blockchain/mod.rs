//! Blockchain fetched from Katana.
use std::collections::HashMap;

use blockifier::block_context::{BlockContext, BlockInfo, ChainInfo, FeeTokenAddresses, GasPrices};
use katana_executor::blockifier::state::CachedStateWrapper;
use blockifier::transaction::objects::TransactionExecutionInfo;
use katana_executor::blockifier::TransactionExecutor;
use katana_executor::blockifier::state::StateRefDb;
use katana_primitives::receipt::Receipt;
use katana_primitives::block::{BlockHashOrNumber, BlockIdOrTag, BlockTag, SealedBlockWithStatus, SealedBlock};
use katana_executor::blockifier::outcome::TxReceiptWithExecInfo;
use katana_primitives::chain::ChainId;
use katana_primitives::transaction::{Tx, TxWithHash, ExecutableTxWithHash};
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_provider::traits::block::{BlockProvider, BlockWriter};
use katana_provider::traits::contract::ContractClassWriter;
use katana_provider::traits::env::BlockEnvProvider;
use katana_provider::traits::state::{
    StateFactoryProvider, StateProvider, StateRootProvider, StateWriter,
};
use katana_provider::traits::state_update::StateUpdateProvider;
use katana_provider::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider, TransactionsProviderExt,
};
use katana_provider::BlockchainProvider;
use starknet_api::block::{BlockNumber, BlockTimestamp};

use crate::error::{Error as SayaError, SayaResult};

const LOG_TARGET: &str = "blockchain";

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
    + BlockEnvProvider
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
        + BlockEnvProvider
        + 'static
        + Send
        + Sync
{
}

/// Represents the whole blockchain fetched from Katana.
pub struct Blockchain {
    inner: BlockchainProvider<Box<dyn Database>>,
}

impl Blockchain {
    /// Initializes a new instance of [`Blockchain`].
    pub fn new() -> Self {
        Self { inner: BlockchainProvider::new(Box::new(InMemoryProvider::new())) }
    }

    /// Returns the internal provider.
    pub fn provider(&self) -> &BlockchainProvider<Box<dyn Database>> {
        &self.inner
    }

    /// Retrieves historical state for the given block.
    ///
    /// # Arguments
    ///
    /// * `block_id` - The block id at which the state must be retrieved.
    pub fn state(&self, block_id: &BlockIdOrTag) -> SayaResult<Box<dyn StateProvider>> {
        let provider = self.provider();

        match block_id {
            BlockIdOrTag::Tag(BlockTag::Latest) => {
                let state = StateFactoryProvider::latest(provider)?;
                Ok(state)
            }

            BlockIdOrTag::Hash(hash) => {
                StateFactoryProvider::historical(provider, BlockHashOrNumber::Hash(*hash))?
                    .ok_or(SayaError::BlockNotFound(*block_id))
            }

            BlockIdOrTag::Number(num) => {
                StateFactoryProvider::historical(provider, BlockHashOrNumber::Num(*num))?
                    .ok_or(SayaError::BlockNotFound(*block_id))
            }

            BlockIdOrTag::Tag(BlockTag::Pending) => {
                panic!("Pending block is not supported");
            }
        }
    }

    /// Updates the [`Blockchain`] internal state adding the given [`SealedBlockWithStatus`]
    /// and the associated [`StateUpdatesWithDeclaredClasses`].
    ///
    /// Currently receipts are ignored.
    ///
    /// # Arguments
    ///
    /// * `block` - The block to add.
    /// * `states` - The state updates associated with the block.
    pub fn update_state_with_block(
        &mut self,
        block: SealedBlockWithStatus,
        states: StateUpdatesWithDeclaredClasses,
    ) -> SayaResult<()> {
        let provider = self.provider();
        // Receipts are not supported currently. We may need them if some
        // information about the transaction is missing.
        let receipts = vec![];

        Ok(provider.insert_block_with_states_and_receipts(block, states, receipts)?)
    }

    /// TEMP: initializes a default block context of the blockifier.
    /// TODO: must be init from a SealedBlockHeader.
    pub fn block_context_default(&self) -> BlockContext {
        let fee_token_addresses = FeeTokenAddresses {
            eth_fee_token_address: 0_u128.into(),
            strk_fee_token_address: 0_u128.into(),
        };

        let gas_prices = GasPrices {
            eth_l1_gas_price: 0,
            strk_l1_gas_price: 0,
            eth_l1_data_gas_price: 0,
            strk_l1_data_gas_price: 0,
        };

        BlockContext {
            block_info: BlockInfo {
                gas_prices,
                block_number: BlockNumber(0),
                block_timestamp: BlockTimestamp(0),
                sequencer_address: 0_u128.into(),
                vm_resource_fee_cost: HashMap::new().into(),
                validate_max_n_steps: 100000000,
                invoke_tx_max_n_steps: 100000000,
                max_recursion_depth: 100,
                use_kzg_da: false,
            },
            chain_info: ChainInfo {
                fee_token_addresses,
                chain_id: ChainId::parse("KATANA").unwrap().into(),
            },
        }
    }

    /// Executes the transactions against the given state to retrieve
    /// the transaction execution info.
    pub fn execute_transactions(&self, block: &SealedBlock) -> SayaResult<()> {
        let provider = self.provider();

        let block_number = block.header.header.number;

        // TODO: get context from block header.
        let block_context = self.block_context_default();

        let state_reader = self.state(&BlockIdOrTag::Number(block_number - 1))?;
        let state: CachedStateWrapper<StateRefDb> = CachedStateWrapper::new(state_reader.into());

        // TODO: from config based on katana config?
        let disable_fee = false;
        let disable_validate = false;

        let mut exec_txs: Vec<ExecutableTxWithHash> = vec![];
        for tx_with_hash in &block.body {
            match &tx_with_hash.transaction {
                Tx::Invoke(t) => exec_txs.push(ExecutableTxWithHash {
                    hash: tx_with_hash.hash,
                    transaction: (*t).clone().into(),
                }),
                // TODO others.
                _ => {}
            }
        }

        let tx_receipt_pairs: Vec<TransactionExecutionInfo> = TransactionExecutor::new(
            &state,
            &block_context,
            !disable_fee,
            !disable_validate,
            exec_txs.into_iter(),
        )
            .with_error_log()
            .with_events_log()
            .with_resources_log()
            .filter_map(|res| {
                if let Ok(info) = res {
                    Some(info)
                } else {
                    None
                }
            })
            .collect();

        Ok(())
    }
}
