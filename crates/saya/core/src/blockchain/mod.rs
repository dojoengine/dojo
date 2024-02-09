//! Blockchain fetched from Katana.
use std::path::Path;
use std::collections::HashMap;

use starknet_api::block::{BlockNumber, BlockTimestamp};
use blockifier::block_context::{BlockInfo, ChainInfo, FeeTokenAddresses, GasPrices, BlockContext};
use blockifier::fee::fee_utils::{calculate_l1_gas_by_vm_usage, extract_l1_gas_and_vm_usage};
use blockifier::state::state_api::State;
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::{
    DeprecatedAccountTransactionContext, ResourcesMapping, TransactionExecutionInfo,
};

use katana_db::init_db;
use katana_primitives::conversion::rpc as rpc_converter;
use katana_primitives::block::{BlockHash, FinalityStatus, SealedBlockWithStatus};
use katana_primitives::contract::ClassHash;
use katana_primitives::genesis::Genesis;
use katana_primitives::chain::ChainId;
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_primitives::block::{BlockTag, BlockIdOrTag, BlockHashOrNumber};
use katana_provider::traits::block::{BlockProvider, BlockWriter};
use katana_provider::traits::contract::ContractClassWriter;
use katana_provider::traits::env::BlockEnvProvider;
use katana_provider::traits::state::{StateFactoryProvider, StateRootProvider, StateWriter, StateProvider};
use katana_provider::traits::state_update::StateUpdateProvider;
use katana_provider::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider, TransactionsProviderExt,
};
use katana_provider::BlockchainProvider;
use starknet::core::types::{
    ContractStorageDiffItem, DeclaredClassItem, DeployedContractItem, FieldElement, NonceUpdate,
    StateDiff, ContractClass,
};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use tracing::{error, trace};

use crate::error::{SayaResult, Error as SayaError};

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
    /// Use a simple in memory db for now. TBD the final schema,
    /// but we don't need to support as much as katana.
    pub fn new() -> Self {
        Self { inner: BlockchainProvider::new(Box::new(InMemoryProvider::new())) }
    }

    pub fn provider(&self) -> &BlockchainProvider<Box<dyn Database>> {
        &self.inner
    }

    pub fn init_from_state_diff(&mut self, state_diff: &StateDiff) -> SayaResult<()> {
        trace!(target: LOG_TARGET,
               storage_updates = &state_diff.storage_diffs.len(),
               nonce_updates = &state_diff.nonces.len(),
               deployed_updates = &state_diff.deployed_contracts.len(),
               declared_updates = &state_diff.declared_classes.len(),
               "genesis updates");

        for contract_diff in &state_diff.storage_diffs {
            let ContractStorageDiffItem { address, storage_entries: entries } = contract_diff;

            for e in entries {
                self.inner.set_storage((*address).into(), e.key, e.value)?;
            }
        }

        for nonce_update in &state_diff.nonces {
            let NonceUpdate { contract_address, nonce: new_nonce } = *nonce_update;
            self.inner.set_nonce(contract_address.into(), new_nonce)?;
        }

        for deployed in &state_diff.deployed_contracts {
            let DeployedContractItem { address, class_hash } = *deployed;
            self.inner.set_class_hash_of_contract(address.into(), class_hash)?;
        }

        for decl in &state_diff.declared_classes {
            let DeclaredClassItem { class_hash, compiled_class_hash } = decl;
            self.inner.set_compiled_class_hash_of_class_hash(
                (*class_hash).into(),
                *compiled_class_hash,
            )?;
        }

        Ok(())
    }

    pub fn set_contract_classes(&mut self, contract_classes: &HashMap<ClassHash, ContractClass>) -> SayaResult<()> {
        for (class_hash, class) in contract_classes {
            match class {
                ContractClass::Legacy(legacy) => {
                    trace!(
                        target: LOG_TARGET,
                        version = "cairo 0",
                        %class_hash,
                        "set contract class");

                    let (hash, class) = rpc_converter::legacy_rpc_to_inner_compiled_class(legacy)?;
                    self.inner.set_class(hash, class)?;
                }
                ContractClass::Sierra(s) => {
                    trace!(
                        target: LOG_TARGET,
                        version = "cairo 1",
                        %class_hash,
                        "set contract class");

                    self.inner.set_sierra_class(*class_hash, s.clone())?;
                }
            }
        }

        Ok(())
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
            chain_info: ChainInfo { fee_token_addresses, chain_id: ChainId::parse("KATANA").unwrap().into() },
        }
    }
}
