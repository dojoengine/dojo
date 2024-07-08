use std::fmt::Display;
use std::path::PathBuf;

use async_trait::async_trait;
use starknet::core::types::Felt;

use super::{Declarable, MigrationType, StateDiff};

/// Represents differences between a local and remote class.
#[derive(Debug, Default, Clone)]
pub struct ClassDiff {
    // The tag is used to identify the corresponding artifact produced by the compiler.
    pub tag: String,
    pub local_class_hash: Felt,
    pub original_class_hash: Felt,
    pub remote_class_hash: Option<Felt>,
}

impl StateDiff for ClassDiff {
    fn is_same(&self) -> bool {
        if let Some(remote) = self.remote_class_hash {
            self.local_class_hash == remote
        } else {
            false
        }
    }
}

impl Display for ClassDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.tag)?;
        writeln!(f, "   Local: {:#x}", self.local_class_hash)?;

        if let Some(remote) = self.remote_class_hash {
            writeln!(f, "   Remote: {remote:#x}")?;
        }

        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct ClassMigration {
    pub diff: ClassDiff,
    pub artifact_path: PathBuf,
}

impl ClassMigration {
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
impl Declarable for ClassMigration {
    fn artifact_path(&self) -> &PathBuf {
        &self.artifact_path
    }
}
