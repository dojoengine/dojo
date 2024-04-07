use std::fmt::Display;
use std::path::PathBuf;

use async_trait::async_trait;
use starknet::core::types::{DeclareTransactionResult, FieldElement};

use super::{Declarable, Deployable, MigrationType, StateDiff, Upgradable};

pub type DeclareOutput = DeclareTransactionResult;

/// Represents differences between a local and remote contract.
#[derive(Debug, Default, Clone)]
pub struct ContractDiff {
    pub name: String,
    pub local_class_hash: FieldElement,
    pub original_class_hash: FieldElement,
    pub base_class_hash: FieldElement,
    pub remote_class_hash: Option<FieldElement>,
}

impl StateDiff for ContractDiff {
    fn is_same(&self) -> bool {
        if let Some(remote) = self.remote_class_hash {
            self.local_class_hash == remote
        } else {
            false
        }
    }
}

impl Display for ContractDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.name)?;
        writeln!(f, "   Local Class Hash: {:#x}", self.local_class_hash)?;
        writeln!(f, "   Original Class Hash: {:#x}", self.original_class_hash)?;
        writeln!(f, "   Base Class Hash: {:#x}", self.base_class_hash)?;

        if let Some(remote) = self.remote_class_hash {
            writeln!(f, "   Remote Class Hash: {remote:#x}")?;
        }

        Ok(())
    }
}

// Represents a contract that needs to be migrated to the remote state
#[derive(Debug, Default)]
pub struct ContractMigration {
    pub salt: FieldElement,
    pub diff: ContractDiff,
    pub artifact_path: PathBuf,
    pub contract_address: FieldElement,
}

impl ContractMigration {
    pub fn migration_type(&self) -> MigrationType {
        let Some(remote) = self.diff.remote_class_hash else {
            return MigrationType::New;
        };

        match self.diff.local_class_hash == remote {
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
    fn salt(&self) -> FieldElement {
        self.salt
    }
}

#[async_trait]
impl Upgradable for ContractMigration {}
