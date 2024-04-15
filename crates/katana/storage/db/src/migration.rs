use std::path::Path;

use anyhow::{anyhow, Context};

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
/// Database migration can only be done on a supported older version of the database schema,
/// meaning not all older versions can be migrated from.
pub fn migrate_db<P: AsRef<Path>>(path: P) -> Result<(), DatabaseMigrationError> {
    // check that the db version is supported, otherwise return an error
    let ver = get_db_version(&path)?;

    match ver {
        0 => migrate_from_v0_to_v1(open_db(&path)?)?,
        _ => {
            return Err(DatabaseMigrationError::UnsupportedVersion(ver));
        }
    }

    // Update the db version to the current version
    create_db_version_file(path, CURRENT_DB_VERSION).context("Updating database version file")?;
    Ok(())
}

/// Perform migration for database version 0 to version 1.
fn migrate_from_v0_to_v1(env: DbEnv) -> Result<(), DatabaseMigrationError> {
    env.create_tables()?;

    env.update(|tx| {
        {
            let mut cursor = tx.cursor::<tables::v0::StorageChangeSet>()?;
            cursor.walk(None)?.try_for_each(|entry| {
                let (old_key, old_val) = entry?;
                let key = ContractStorageKey { contract_address: old_key, key: old_val.key };
                tx.put::<tables::StorageChangeSet>(key, BlockList::from_iter(old_val.block_list))?;
                Result::<(), DatabaseError>::Ok(())
            })?;

            // move data from `NonceChanges` to `NonceChangeHistory`
            let mut cursor = tx.cursor::<tables::v0::NonceChanges>()?;
            cursor.walk(None)?.try_for_each(|entry| {
                let (key, val) = entry?;
                tx.put::<tables::NonceChangeHistory>(key, val)?;
                Result::<(), DatabaseError>::Ok(())
            })?;

            // move data from `StorageChanges` to `StorageChangeHistory`
            let mut cursor = tx.cursor::<tables::v0::StorageChanges>()?;
            cursor.walk(None)?.try_for_each(|entry| {
                let (key, val) = entry?;
                tx.put::<tables::StorageChangeHistory>(key, val)?;
                Result::<(), DatabaseError>::Ok(())
            })?;

            // move data from `ContractClassChanges` to `ClassChangeHistory`
            let mut cursor = tx.cursor::<tables::v0::ContractClassChanges>()?;
            cursor.walk(None)?.try_for_each(|entry| {
                let (key, val) = entry?;
                tx.put::<tables::ClassChangeHistory>(key, val)?;
                Result::<(), DatabaseError>::Ok(())
            })?;
        }

        // drop the old tables
        unsafe {
            tx.drop_table::<tables::v0::StorageChangeSet>()?;
            tx.drop_table::<tables::v0::NonceChanges>()?;
            tx.drop_table::<tables::v0::StorageChanges>()?;
            tx.drop_table::<tables::v0::ContractClassChanges>()?;
        }

        Ok(())
    })?
}

#[cfg(test)]
mod tests {

    use crate::{init_db, mdbx::DbEnv, open_db, version::create_db_version_file};
    use std::path::PathBuf;

    use super::migrate_db;

    const ERROR_CREATE_TEMP_DIR: &str = "Failed to create temp dir.";
    const ERROR_MIGRATE_DB: &str = "Failed to migrate db.";
    const ERROR_INIT_DB: &str = "Failed to initialize db.";
    const ERROR_CREATE_TABLES: &str = "Failed to create tables.";
    const ERROR_CREATE_VER_FILE: &str = "Failed to create version file.";

    fn create_test_db() -> (DbEnv, PathBuf) {
        let path = tempfile::TempDir::new().expect(ERROR_CREATE_TEMP_DIR).into_path();
        let db = init_db(&path).expect(ERROR_INIT_DB);
        (db, path)
    }

    fn create_v0_test_db() -> (DbEnv, PathBuf) {
        let path = tempfile::TempDir::new().expect(ERROR_CREATE_TEMP_DIR).into_path();

        let db = open_db(&path).expect(ERROR_INIT_DB);
        let _ = db.create_v0_tables().expect(ERROR_CREATE_TABLES);
        let _ = create_db_version_file(&path, 0).expect(ERROR_CREATE_VER_FILE);

        (db, path)
    }

    #[test]
    fn migrate_from_current_version() {
        let (_, path) = create_test_db();
        assert_eq!(
            migrate_db(path).unwrap_err().to_string(),
            "Unsupported database version for migration: 1",
            "Can't migrate from the current version"
        );
    }

    #[test]
    fn migrate_from_v0() {
        let (env, path) = create_v0_test_db();
        let _ = migrate_db(path).expect(ERROR_MIGRATE_DB);
    }
}
