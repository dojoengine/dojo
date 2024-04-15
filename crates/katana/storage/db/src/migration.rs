use std::path::Path;

use anyhow::Context;

use crate::{
    error::DatabaseError,
    mdbx::DbEnv,
    models::{list::BlockList, storage::ContractStorageKey},
    open_db, tables,
    version::{create_db_version_file, get_db_version, DatabaseVersionError},
    CURRENT_DB_VERSION,
};

#[derive(Debug, thiserror::Error)]
pub enum DatabaseMigrationError {
    #[error("Unsupported database version for migration: {0}")]
    UnsupportedVersion(u32),

    #[error(transparent)]
    DatabaseVersion(#[from] DatabaseVersionError),

    #[error(transparent)]
    Database(#[from] DatabaseError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Performs a database migration for an already initialized database with an older
/// version of the database schema.
///
/// DB migration can only be done on a supported older version of the database schema,
/// meaning not all older versions can be migrated.
pub fn migrate_db<P: AsRef<Path>>(path: P) -> Result<(), DatabaseMigrationError> {
    // check that the db version is supported, otherwise return an error
    let ver = get_db_version(&path)?;

    if ver == 0 {
        // perform the migration
        // 1. create all the tables exist in the current schema
        // 2. migrate all the data from the old schema to the new schema
        let db = open_db(&path)?;
        migrate_from_v0_to_v1(db)?;
    } else {
        return Err(DatabaseMigrationError::UnsupportedVersion(ver));
    }

    // Update the db version to the current version
    create_db_version_file(path, CURRENT_DB_VERSION).context("Updating database version file")?;
    Ok(())
}

fn migrate_from_v0_to_v1(env: DbEnv) -> Result<(), DatabaseMigrationError> {
    env.create_tables()?;
    env.update(|tx| {
        // migrate the block list
        let mut cursor = tx.cursor::<tables::v0::StorageChangeSet>()?;
        let walker = cursor.walk(None)?;
        for old_entry in walker {
            let (old_key, old_val) = old_entry?;

            let key = ContractStorageKey { contract_address: old_key, key: old_val.key };
            let value = BlockList::from_iter(old_val.block_list);

            tx.put::<tables::StorageChangeSet>(key, value)?;
        }

        drop(cursor);

        // drop the old table
        unsafe {
            tx.drop_table::<tables::v0::StorageChangeSet>()?;
        }

        Ok(())
    })?
}
