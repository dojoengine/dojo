//! Blockchain fetched from Katana.
use std::collections::HashMap;
use std::path::Path;

use blockifier::block_context::{BlockContext, BlockInfo, ChainInfo, FeeTokenAddresses, GasPrices};
use blockifier::fee::fee_utils::{calculate_l1_gas_by_vm_usage, extract_l1_gas_and_vm_usage};
use blockifier::state::state_api::State;
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::{
    DeprecatedAccountTransactionContext, ResourcesMapping, TransactionExecutionInfo,
};
use katana_db::init_db;
use katana_primitives::block::{
    BlockHash, BlockHashOrNumber, BlockIdOrTag, BlockTag, FinalityStatus, SealedBlockWithStatus,
};
use katana_primitives::chain::ChainId;
use katana_primitives::contract::ClassHash;
use katana_primitives::conversion::rpc as rpc_converter;
use katana_primitives::genesis::Genesis;
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
use starknet::core::types::{
    ContractClass, ContractStorageDiffItem, DeclaredClassItem, DeployedContractItem, FieldElement,
    NonceUpdate, StateDiff,
};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use starknet_api::block::{BlockNumber, BlockTimestamp};
use tracing::{error, trace};

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

pub struct Blockchain {
    inner: BlockchainProvider<Box<dyn Database>>,
}

impl Blockchain {
    pub fn new() -> Self {
        Self { inner: BlockchainProvider::new(Box::new(InMemoryProvider::new())) }
    }

    pub fn provider(&self) -> &BlockchainProvider<Box<dyn Database>> {
        &self.inner
    }

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
}
