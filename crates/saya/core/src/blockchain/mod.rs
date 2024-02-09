//! Blockchain fetched from Katana.
use std::path::Path;
use std::collections::HashMap;

use katana_db::init_db;
use katana_primitives::conversion::rpc as rpc_converter;
use katana_primitives::block::{BlockHash, FinalityStatus, SealedBlockWithStatus};
use katana_primitives::contract::ClassHash;
use katana_primitives::genesis::Genesis;
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_provider::traits::block::{BlockProvider, BlockWriter};
use katana_provider::traits::contract::ContractClassWriter;
use katana_provider::traits::env::BlockEnvProvider;
use katana_provider::traits::state::{StateFactoryProvider, StateRootProvider, StateWriter};
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

use crate::error::SayaResult;

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

                    println!("{:?}", legacy);

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
}
