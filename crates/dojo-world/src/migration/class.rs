use std::fmt::Display;
use std::path::PathBuf;

use async_trait::async_trait;
use starknet::core::types::FieldElement;

use super::{Declarable, MigrationType, StateDiff};

/// Represents differences between a local and remote class.
#[derive(Debug, Default, Clone)]
pub struct ClassDiff {
    pub name: String,
    pub local: FieldElement,
    pub remote: Option<FieldElement>,
}

impl StateDiff for ClassDiff {
    fn is_same(&self) -> bool {
        if let Some(remote) = self.remote {
            self.local == remote
        } else {
            false
        }
    }
}

impl Display for ClassDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.name)?;
        writeln!(f, "   Local: {:#x}", self.local)?;

        if let Some(remote) = self.remote {
            writeln!(f, "   Remote: {remote:#x}")?;
        }

        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct ClassMigration {
    pub diff: ClassDiff,
    pub artifact_path: PathBuf,
}

impl ClassMigration {
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
impl Declarable for ClassMigration {
    fn artifact_path(&self) -> &PathBuf {
        &self.artifact_path
    }
}
