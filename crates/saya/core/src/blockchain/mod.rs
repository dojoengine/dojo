//! Blockchain fetched from Katana.
use std::collections::HashMap;

use blockifier::block_context::{BlockContext, BlockInfo, GasPrices};
use blockifier::transaction::objects::TransactionExecutionInfo;
use cairo_vm::vm::runners::builtin_runner::{
    BITWISE_BUILTIN_NAME, EC_OP_BUILTIN_NAME, HASH_BUILTIN_NAME, KECCAK_BUILTIN_NAME,
    OUTPUT_BUILTIN_NAME, POSEIDON_BUILTIN_NAME, RANGE_CHECK_BUILTIN_NAME,
    SEGMENT_ARENA_BUILTIN_NAME, SIGNATURE_BUILTIN_NAME,
};
use katana_executor::blockifier::state::{CachedStateWrapper, StateRefDb};
use katana_executor::blockifier::TransactionExecutor;
use katana_primitives::block::{
    BlockHashOrNumber, BlockIdOrTag, BlockTag, SealedBlock, SealedBlockWithStatus, SealedHeader,
};
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_primitives::transaction::{ExecutableTxWithHash, Tx};
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

const MAX_RECURSION_DEPTH: usize = 1000;

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

impl Default for Blockchain {
    fn default() -> Self {
        Self::new()
    }
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

        Ok(provider.insert_block_with_states_and_receipts(block, states, receipts, vec![])?)
    }

    /// Executes the transactions against the given state to retrieve
    /// the transaction execution info.
    ///
    /// TODO: to be replaced by Katana endpoint that exposes the execution info
    /// for all transactions in a block.
    pub fn execute_transactions(
        &self,
        block: &SealedBlock,
        block_context: &BlockContext,
    ) -> SayaResult<Vec<TransactionExecutionInfo>> {
        let block_number = block.header.header.number;
        let state_reader = self.state(&BlockIdOrTag::Number(block_number - 1))?;
        let state: CachedStateWrapper<StateRefDb> = CachedStateWrapper::new(state_reader.into());

        let mut exec_txs: Vec<ExecutableTxWithHash> = vec![];
        for tx_with_hash in &block.body {
            match &tx_with_hash.transaction {
                Tx::Invoke(t) => exec_txs.push(ExecutableTxWithHash {
                    hash: tx_with_hash.hash,
                    transaction: (*t).clone().into(),
                }),
                Tx::L1Handler(t) => exec_txs.push(ExecutableTxWithHash {
                    hash: tx_with_hash.hash,
                    transaction: (*t).clone().into(),
                }),
                Tx::DeployAccount(t) => exec_txs.push(ExecutableTxWithHash {
                    hash: tx_with_hash.hash,
                    transaction: (*t).clone().into(),
                }),
                // TODO DECLARE with class.
                _ => {}
            }
        }

        // TODO: this must be the same as katana.
        let disable_fee = false;
        let disable_validate = false;

        let exec_infos: Vec<TransactionExecutionInfo> = TransactionExecutor::new(
            &state,
            block_context,
            !disable_fee,
            !disable_validate,
            exec_txs.into_iter(),
        )
        .with_error_log()
        .with_events_log()
        .with_resources_log()
        .filter_map(|res| if let Ok(info) = res { Some(info) } else { None })
        .collect();

        Ok(exec_infos)
    }
}

/// Initializes a [`BlockInfo`] from a [`SealedHeader`] and additional information.
///
/// # Arguments
///
/// * `header` - The header to get information from.
/// * `invoke_tx_max_n_steps` - Maximum number of steps for invoke tx.
/// * `validate_max_n_steps` - Maximum number of steps to validate a tx.
pub fn block_info_from_header(
    header: &SealedHeader,
    invoke_tx_max_n_steps: u32,
    validate_max_n_steps: u32,
) -> BlockInfo {
    let gas_prices = GasPrices {
        eth_l1_gas_price: header.header.gas_prices.eth as u128,
        strk_l1_gas_price: header.header.gas_prices.strk as u128,
        eth_l1_data_gas_price: 0,
        strk_l1_data_gas_price: 0,
    };

    BlockInfo {
        gas_prices,
        block_number: BlockNumber(header.header.number),
        block_timestamp: BlockTimestamp(header.header.timestamp),
        sequencer_address: header.header.sequencer_address.into(),
        vm_resource_fee_cost: get_default_vm_resource_fee_cost().into(),
        validate_max_n_steps,
        invoke_tx_max_n_steps,
        max_recursion_depth: MAX_RECURSION_DEPTH,
        use_kzg_da: false,
    }
}

fn get_default_vm_resource_fee_cost() -> HashMap<String, f64> {
    HashMap::from([
        (String::from("n_steps"), 1_f64),
        (HASH_BUILTIN_NAME.to_string(), 1_f64),
        (RANGE_CHECK_BUILTIN_NAME.to_string(), 1_f64),
        (SIGNATURE_BUILTIN_NAME.to_string(), 1_f64),
        (BITWISE_BUILTIN_NAME.to_string(), 1_f64),
        (POSEIDON_BUILTIN_NAME.to_string(), 1_f64),
        (OUTPUT_BUILTIN_NAME.to_string(), 1_f64),
        (EC_OP_BUILTIN_NAME.to_string(), 1_f64),
        (KECCAK_BUILTIN_NAME.to_string(), 1_f64),
        (SEGMENT_ARENA_BUILTIN_NAME.to_string(), 1_f64),
    ])
}
