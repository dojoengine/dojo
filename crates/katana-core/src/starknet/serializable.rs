use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use std::sync::Arc;

use blockifier::block_context::BlockContext;
use blockifier::execution::contract_class::{
    ContractClass, ContractClassV0, ContractClassV1, EntryPointV1,
};
use blockifier::state::cached_state::CachedState;
use cairo_lang_casm::hints::Hint;
use cairo_vm::types::errors::program_errors::ProgramError;
use serde::{Deserialize, Serialize};
use starknet_api::deprecated_contract_class::{EntryPoint, EntryPointType};
use starknet_api::hash::StarkFelt;

use super::block::StarknetBlocks;
use super::transaction::StarknetTransactions;
use super::{StarknetConfig, StarknetWrapper};
use crate::accounts::{Account, PredeployedAccounts};
use crate::state::DictStateReader;

pub trait SerializableState {
    fn dump_state(&self, path: &PathBuf) -> std::io::Result<()>;

    fn load_state(&mut self, path: &PathBuf) -> std::io::Result<()>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SerializableStarknetWrapper {
    pub config: StarknetConfig,
    pub blocks: StarknetBlocks,
    pub block_context: BlockContext,
    pub transactions: StarknetTransactions,
    pub state: DictStateReader,
    pub predeployed_accounts: SerializablePredeployedAccounts,
    pub pending_state: CachedState<DictStateReader>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializablePredeployedAccounts {
    pub seed: [u8; 32],
    pub accounts: Vec<Account>,
    pub initial_balance: StarkFelt,
    pub contract_class: SerializableContractClass,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SerializableContractClassV0(pub Arc<SerializableContractClassV0Inner>);

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SerializableContractClassV0Inner {
    pub program: String, // Program serialized as a String
    pub entry_points_by_type: HashMap<EntryPointType, Vec<EntryPoint>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SerializableContractClassV1(pub Arc<SerializableContractClassV1Inner>);

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SerializableContractClassV1Inner {
    pub program: String, // Program serialized as a String
    pub entry_points_by_type: HashMap<EntryPointType, Vec<EntryPointV1>>,
    pub hints: HashMap<String, Hint>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SerializableContractClass {
    V0(SerializableContractClassV0),
    V1(SerializableContractClassV1),
}

impl TryFrom<SerializableStarknetWrapper> for StarknetWrapper {
    type Error = ProgramError;
    fn try_from(wrapper: SerializableStarknetWrapper) -> Result<Self, ProgramError> {
        let SerializableStarknetWrapper {
            config,
            blocks,
            block_context,
            transactions,
            state,
            predeployed_accounts,
            pending_state,
        } = wrapper;

        Ok(Self {
            config,
            blocks,
            block_context,
            transactions,
            state,
            predeployed_accounts: predeployed_accounts.try_into()?,
            pending_state,
        })
    }
}

impl TryFrom<SerializablePredeployedAccounts> for PredeployedAccounts {
    type Error = ProgramError;
    fn try_from(accounts: SerializablePredeployedAccounts) -> Result<Self, ProgramError> {
        let SerializablePredeployedAccounts { seed, accounts, initial_balance, contract_class } =
            accounts;

        Ok(Self { seed, accounts, initial_balance, contract_class: contract_class.try_into()? })
    }
}

impl TryFrom<SerializableContractClass> for ContractClass {
    type Error = ProgramError;
    fn try_from(contract_class: SerializableContractClass) -> Result<Self, ProgramError> {
        match contract_class {
            SerializableContractClass::V0(contract_class) => {
                Ok(ContractClass::V0(contract_class.try_into()?))
            }
            SerializableContractClass::V1(contract_class) => {
                Ok(ContractClass::V1(contract_class.try_into()?))
            }
        }
    }
}

impl TryFrom<SerializableContractClassV0> for ContractClassV0 {
    type Error = ProgramError;
    fn try_from(contract_class: SerializableContractClassV0) -> Result<Self, ProgramError> {
        let contract_class_v0 = ContractClassV0::try_from_json_string(&contract_class.0.program)?;
        Ok(contract_class_v0)
    }
}

impl TryFrom<SerializableContractClassV1> for ContractClassV1 {
    type Error = ProgramError;
    fn try_from(contract_class: SerializableContractClassV1) -> Result<Self, ProgramError> {
        let contract_class_v1 = ContractClassV1::try_from_json_string(&contract_class.0.program)?;
        Ok(contract_class_v1)
    }
}

impl SerializableState for SerializableStarknetWrapper {
    fn dump_state(&self, path: &PathBuf) -> std::io::Result<()> {
        let encoded: Vec<u8> = bincode::serialize(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(&encoded)?;
        Ok(())
    }

    fn load_state(&mut self, path: &PathBuf) -> std::io::Result<()> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;

        // decode buffer content
        let decoded: SerializableStarknetWrapper = bincode::deserialize(&buffer)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        *self = decoded;
        Ok(())
    }
}
