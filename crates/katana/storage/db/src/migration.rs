use std::path::Path;

use crate::{
    error::DatabaseError,
    open_db, tables,
    version::{get_db_version, DatabaseVersionError},
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

    if ver != 0 {
        return Err(DatabaseMigrationError::UnsupportedVersion(ver));
    }

    // perform the migration
    // 1. create all the tables exist in the current schema
    // 2. migrate all the data from the old schema to the new schema
    // 3. update the db version to the current version

    let env = open_db(path)?;
    env.create_tables()?;

    env.update(|tx| {
        let mut cursor = tx.cursor::<tables::StorageChangeSet>()?;
    })?;

    Ok(())
}
