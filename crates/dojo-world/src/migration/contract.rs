use std::fmt::Display;
use std::path::PathBuf;

use async_trait::async_trait;
use starknet::core::types::{DeclareTransactionResult, FieldElement};

use super::{Declarable, Deployable, MigrationType, StateDiff};

pub type DeclareOutput = DeclareTransactionResult;

/// Represents differences between a local and remote contract.
#[derive(Debug, Default, Clone)]
pub struct ContractDiff {
    pub name: String,
    pub local: FieldElement,
    pub remote: Option<FieldElement>,
    pub address: Option<FieldElement>,
}

impl StateDiff for ContractDiff {
    fn is_same(&self) -> bool {
        if let Some(remote) = self.remote {
            self.local == remote
        } else {
            false
        }
    }
}

impl Display for ContractDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.name)?;
        if let Some(address) = self.address {
            writeln!(f, "   Address: {address:#x}",)?;
        }
        writeln!(f, "   Local: {:#x}", self.local)?;

        if let Some(remote) = self.remote {
            writeln!(f, "   Remote: {remote:#x}")?;
        }

        Ok(())
    }
}

// TODO: evaluate the contract address when building the migration plan
// Represents a contract that needs to be migrated to the remote state
#[derive(Debug, Default)]
pub struct ContractMigration {
    pub salt: FieldElement,
    pub diff: ContractDiff,
    pub artifact_path: PathBuf,
    pub contract_address: Option<FieldElement>,
}

impl ContractMigration {
    pub fn migration_type(&self) -> MigrationType {
        let Some(remote ) = self.diff.remote else {
            return MigrationType::New;
        };

        match self.diff.local == remote {
            true => MigrationType::New,
            false => MigrationType::Update,
        }
    }
}

#[async_trait]
impl Declarable for ContractMigration {
    fn artifact_path(&self) -> &PathBuf {
        &self.artifact_path
    }
}

#[async_trait]
impl Deployable for ContractMigration {
    fn set_contract_address(&mut self, contract_address: FieldElement) {
        self.contract_address = Some(contract_address);
    }
}
