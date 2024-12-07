//! Code adapted from Paradigm's [`reth`](https://github.com/paradigmxyz/reth/tree/main/crates/storage/db) DB implementation.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context};

pub mod abstraction;
pub mod codecs;
pub mod error;
pub mod mdbx;
pub mod models;
pub mod tables;
pub mod trie;
pub mod utils;
pub mod version;

use mdbx::{DbEnv, DbEnvKind};
use utils::is_database_empty;
use version::{check_db_version, create_db_version_file, DatabaseVersionError, CURRENT_DB_VERSION};

/// Initialize the database at the given path and returning a handle to the its
/// environment.
///
/// This will create the default tables, if necessary.
pub fn init_db<P: AsRef<Path>>(path: P) -> anyhow::Result<DbEnv> {
    if is_database_empty(path.as_ref()) {
        fs::create_dir_all(&path).with_context(|| {
            format!("Creating database directory at path {}", path.as_ref().display())
        })?;
        create_db_version_file(&path, CURRENT_DB_VERSION).with_context(|| {
            format!("Inserting database version file at path {}", path.as_ref().display())
        })?
    } else {
        match check_db_version(&path) {
            Ok(_) => {}
            Err(DatabaseVersionError::FileNotFound) => {
                create_db_version_file(&path, CURRENT_DB_VERSION).with_context(|| {
                    format!(
                        "No database version file found. Inserting version file at path {}",
                        path.as_ref().display()
                    )
                })?
            }
            Err(err) => return Err(anyhow!(err)),
        }
    }

    let env = open_db(path)?;
    env.create_tables()?;
    Ok(env)
}

/// Similar to [`init_db`] but will initialize a temporary database.
///
/// Though it is useful for testing per se, but the initial motivation to implement this
/// variation of database is to be used as the backend for the in-memory storage
/// provider. Mainly to avoid having two separate implementations for the in-memory and
/// persistent db. Simplifying it to using a single solid implementation.
///
/// As such, this database environment will trade off durability for write performance and shouldn't
/// be used in the case where data persistence is required. For that, use [`init_db`].
pub fn init_ephemeral_db() -> anyhow::Result<DbEnv> {
    // Because the underlying database will always be removed, so there's no need to include the
    // version file.
    let env = DbEnv::open_ephemeral().context("Opening ephemeral database")?;
    env.create_tables()?;
    Ok(env)
}

/// Open the database at the given `path` in read-write mode.
pub fn open_db<P: AsRef<Path>>(path: P) -> anyhow::Result<DbEnv> {
    DbEnv::open(path.as_ref(), DbEnvKind::RW).with_context(|| {
        format!("Opening database in read-write mode at path {}", path.as_ref().display())
    })
}

#[cfg(test)]
mod tests {

    use std::fs;

    use crate::version::{default_version_file_path, get_db_version, CURRENT_DB_VERSION};
    use crate::{init_db, init_ephemeral_db};

    #[test]
    fn initialize_db_in_empty_dir() {
        let path = tempfile::tempdir().unwrap();
        init_db(path.path()).unwrap();

        let version_file = fs::File::open(default_version_file_path(path.path())).unwrap();
        let actual_version = get_db_version(path.path()).unwrap();

        assert!(
            version_file.metadata().unwrap().permissions().readonly(),
            "version file should set to read-only"
        );
        assert_eq!(actual_version, CURRENT_DB_VERSION);
    }

    #[test]
    fn initialize_db_in_existing_db_dir() {
        let path = tempfile::tempdir().unwrap();

        init_db(path.path()).unwrap();
        let version = get_db_version(path.path()).unwrap();

        init_db(path.path()).unwrap();
        let same_version = get_db_version(path.path()).unwrap();

        assert_eq!(version, same_version);
    }

    #[test]
    fn initialize_db_with_malformed_version_file() {
        let path = tempfile::tempdir().unwrap();
        let version_file_path = default_version_file_path(path.path());
        fs::write(version_file_path, b"malformed").unwrap();

        let err = init_db(path.path()).unwrap_err();
        assert!(err.to_string().contains("Malformed database version file"));
    }

    #[test]
    fn initialize_db_with_mismatch_version() {
        let path = tempfile::tempdir().unwrap();
        let version_file_path = default_version_file_path(path.path());
        fs::write(version_file_path, 99u32.to_be_bytes()).unwrap();

        let err = init_db(path.path()).unwrap_err();
        assert!(err.to_string().contains("Database version mismatch"));
    }

    #[test]
    fn initialize_db_with_missing_version_file() {
        let path = tempfile::tempdir().unwrap();
        init_db(path.path()).unwrap();

        fs::remove_file(default_version_file_path(path.path())).unwrap();

        init_db(path.path()).unwrap();
        let actual_version = get_db_version(path.path()).unwrap();
        assert_eq!(actual_version, CURRENT_DB_VERSION);
    }

    #[test]
    fn ephemeral_db_deletion_on_drop() {
        // Create an ephemeral database
        let db = init_ephemeral_db().expect("failed to create ephemeral database");
        let dir_path = db.path().to_path_buf();

        // Ensure the directory exists
        assert!(dir_path.exists(), "Database directory should exist");

        // Create a clone of the database to increase the reference count
        let db_clone = db.clone();

        // Drop the original database
        drop(db);

        // Directory should still exist because `db_clone` is still alive
        assert!(
            dir_path.exists(),
            "Database directory should still exist after dropping original reference"
        );

        // Drop the cloned database
        drop(db_clone);

        // Now the directory should be deleted
        assert!(
            !dir_path.exists(),
            "Database directory should be deleted after all references are dropped"
        );
    }
}
