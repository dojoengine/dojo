use std::sync::Arc;

use super::{DEFAULT_MAX_READERS, GIGABYTE};
use crate::error::DatabaseError;
use crate::mdbx::libmdbx::{EnvironmentFlags, Geometry, Mode, PageSize, SyncMode};
use crate::mdbx::DbEnv;
use crate::tables::Tables;
use crate::{abstraction, utils};

/// Temporary database environment that will be deleted when dropped.
///
/// Thought it is useful for testing per se, but the initial motivation to implement this variant of
/// the mdbx environment is to be used as the backend for the in-memory storage provider. Mainly to
/// avoid having two separate implementations for in-memory and persistent db. Therefore, this
/// temporary database environment will trade off durability for write performance.
///
/// It is worth noting that we may change to a different implementation for the in-memory version if
/// we deem that the performance is not satisfactory.
#[derive(Debug, Clone)]
pub struct TempDbEnv {
    /// The mdbx database environment.
    env: DbEnv,
    /// The handle to the temporary directory where the database is stored.
    ///
    /// Wrapped in Arc to allow `TempDbEnv` to be cloned while ensuring
    /// the directory is only dropped when all references are gone.
    _dir: Arc<tempfile::TempDir>,
}

impl TempDbEnv {
    /// Opens a temporary database with the lowest durability settings to favour write throughput.
    ///
    /// This function creates a temporary folder and opens a database in it with minimal durability.
    pub fn open() -> Result<Self, DatabaseError> {
        let _dir = Arc::new(tempfile::tempdir().expect("failed to create temporary db directory"));
        let path = _dir.path();

        let mut builder = libmdbx::Environment::builder();
        builder
            .set_max_dbs(Tables::ALL.len())
            .set_geometry(Geometry {
                size: Some(0..(GIGABYTE * 10)),             // 10gb
                growth_step: Some((GIGABYTE / 2) as isize), // 512mb
                shrink_threshold: None,
                page_size: Some(PageSize::Set(utils::default_page_size())),
            })
            .set_flags(EnvironmentFlags {
                mode: Mode::ReadWrite { sync_mode: SyncMode::UtterlyNoSync }, // we dont care about durability here
                no_rdahead: true,
                coalesce: true,
                ..Default::default()
            })
            .set_max_readers(DEFAULT_MAX_READERS);

        let env = DbEnv(builder.open(path).map_err(DatabaseError::OpenEnv)?).with_metrics();
        Ok(Self { env, _dir })
    }
}

impl std::ops::Deref for TempDbEnv {
    type Target = DbEnv;

    fn deref(&self) -> &Self::Target {
        &self.env
    }
}

impl abstraction::Database for TempDbEnv {
    type Tx = <DbEnv as abstraction::Database>::Tx;
    type TxMut = <DbEnv as abstraction::Database>::TxMut;
    type Stats = <DbEnv as abstraction::Database>::Stats;

    fn tx(&self) -> Result<Self::Tx, DatabaseError> {
        abstraction::Database::tx(&self.env)
    }

    fn tx_mut(&self) -> Result<Self::TxMut, DatabaseError> {
        abstraction::Database::tx_mut(&self.env)
    }

    fn stats(&self) -> Result<Self::Stats, DatabaseError> {
        abstraction::Database::stats(&self.env)
    }
}

impl dojo_metrics::Report for TempDbEnv {
    fn report(&self) {
        dojo_metrics::Report::report(&self.env);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_db_env_open_and_drop() {
        let temp_db = TempDbEnv::open().expect("Failed to open TempDbEnv");
        let temp_dir_path = temp_db._dir.path().to_path_buf();
        assert!(temp_dir_path.exists(), "Temporary directory should exist");

        // Drop the TempDbEnv, this should also remove the temporary directory
        drop(temp_db);

        // At this point, temp_db has been dropped
        assert!(
            !temp_dir_path.exists(),
            "Temporary directory should be deleted after TempDbEnv is dropped"
        );
    }

    #[test]
    fn temp_db_env_clone_and_drop() {
        let temp_db = TempDbEnv::open().expect("Failed to open TempDbEnv");
        let temp_dir_path = temp_db._dir.path().to_path_buf();
        assert!(temp_dir_path.exists(), "Temporary directory should exist");

        // Clone the TempDbEnv
        let temp_db_clone = temp_db.clone();

        // Drop the original TempDbEnv
        drop(temp_db);

        // The temporary directory should still exist because temp_db_clone is still alive
        assert!(
            temp_dir_path.exists(),
            "Temporary directory should still exist after dropping the original"
        );

        // Clone again
        let temp_db_clone2 = temp_db_clone.clone();

        // Drop the first temp_db_clone
        drop(temp_db_clone);

        // The temporary directory should still exist because we still have the second instance of
        // temp_db
        assert!(
            temp_dir_path.exists(),
            "Temporary directory should still exist after dropping the first clone"
        );

        // Drop the last reference
        drop(temp_db_clone2);

        // At this point, all references to TempDbEnv have been dropped
        assert!(
            !temp_dir_path.exists(),
            "Temporary directory should be deleted after all TempDbEnv references are dropped"
        );
    }
}
