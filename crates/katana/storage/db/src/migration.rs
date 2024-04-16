use std::path::Path;

use anyhow::Context;

use crate::error::DatabaseError;
use crate::mdbx::DbEnv;
use crate::models::list::BlockList;
use crate::models::storage::ContractStorageKey;
use crate::version::{create_db_version_file, get_db_version, DatabaseVersionError};
use crate::{open_db, tables, CURRENT_DB_VERSION};

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
    // check that the db version is supported
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
///
/// # Changelog from v0 to v1
///
/// 1. [ContractClassChanges](tables::v0::ContractClassChanges)
/// - Renamed to [ClassChangeHistory](tables::ClassCh
///
/// 2. [StorageChanges](tables::v0::StorageChanges)
/// - Renamed to [StorageChangeHistory](tables::StorageChangeHistory)
///
/// 3. [NonceChanges](tables::v0::NonceChanges)
/// - Renamed to [NonceChangeHistory](tables::NonceChangeHistory)
///
/// 4. [StorageChangeSet](tables::v0::StorageChangeSet)
/// - Changed table type from dupsort to normal table.
/// - Changed key type to [ContractStorageKey](crate::models::storage::ContractStorageKey).
/// - Changed value type to [BlockList](crate::models::list::BlockList).
///
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

    use std::path::PathBuf;

    use starknet::macros::felt;

    use super::migrate_db;
    use crate::mdbx::DbEnv;
    use crate::models::contract::{ContractClassChange, ContractNonceChange};
    use crate::models::list::BlockList;
    use crate::models::storage::{ContractStorageEntry, ContractStorageKey};
    use crate::tables::v0::{self, StorageEntryChangeList};
    use crate::{init_db, tables};

    const ERROR_CREATE_TEMP_DIR: &str = "Failed to create temp dir.";
    const ERROR_MIGRATE_DB: &str = "Failed to migrate db.";
    const ERROR_INIT_DB: &str = "Failed to initialize db.";

    fn create_test_db() -> (DbEnv, PathBuf) {
        let path = tempfile::TempDir::new().expect(ERROR_CREATE_TEMP_DIR).into_path();
        let db = init_db(&path).expect(ERROR_INIT_DB);
        (db, path)
    }

    // TODO(kariy): create Arbitrary for database key/value types to easily create random test vectors
    fn create_v0_test_db() -> (DbEnv<v0::Tables>, PathBuf) {
        let path = tempfile::TempDir::new().expect(ERROR_CREATE_TEMP_DIR).into_path();
        let db = crate::init_db_with_schema::<v0::Tables>(&path).expect(ERROR_INIT_DB);

        db.update(|tx| {
            tx.put::<v0::NonceChanges>(
                1,
                ContractNonceChange { contract_address: felt!("0x1").into(), nonce: felt!("0x2") },
            )
            .unwrap();
            tx.put::<v0::NonceChanges>(
                1,
                ContractNonceChange { contract_address: felt!("0x2").into(), nonce: felt!("0x2") },
            )
            .unwrap();
            tx.put::<v0::NonceChanges>(
                3,
                ContractNonceChange { contract_address: felt!("0x3").into(), nonce: felt!("0x2") },
            )
            .unwrap();

            tx.put::<v0::ContractClassChanges>(
                1,
                ContractClassChange {
                    contract_address: felt!("0x1").into(),
                    class_hash: felt!("0x1"),
                },
            )
            .unwrap();
            tx.put::<v0::ContractClassChanges>(
                1,
                ContractClassChange {
                    contract_address: felt!("0x2").into(),
                    class_hash: felt!("0x1"),
                },
            )
            .unwrap();

            tx.put::<v0::StorageChangeSet>(
                felt!("0x1").into(),
                StorageEntryChangeList { key: felt!("0x1"), block_list: vec![1, 2] },
            )
            .unwrap();
            tx.put::<v0::StorageChangeSet>(
                felt!("0x1").into(),
                StorageEntryChangeList { key: felt!("0x2"), block_list: vec![1, 3] },
            )
            .unwrap();
            tx.put::<v0::StorageChangeSet>(
                felt!("0x2").into(),
                StorageEntryChangeList { key: felt!("0x3"), block_list: vec![4, 5] },
            )
            .unwrap();

            tx.put::<v0::StorageChanges>(
                1,
                ContractStorageEntry {
                    key: ContractStorageKey {
                        contract_address: felt!("0x1").into(),
                        key: felt!("0x1"),
                    },
                    value: felt!("0x1"),
                },
            )
            .unwrap();
            tx.put::<v0::StorageChanges>(
                3,
                ContractStorageEntry {
                    key: ContractStorageKey {
                        contract_address: felt!("0x1").into(),
                        key: felt!("0x2"),
                    },
                    value: felt!("0x2"),
                },
            )
            .unwrap();
        })
        .expect(ERROR_INIT_DB);

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

        env.view(|tx| {
            let mut cursor = tx.cursor::<tables::NonceChangeHistory>().unwrap();
            let val1 = cursor.seek_by_key_subkey(1, felt!("0x1").into()).unwrap();
            let val2 = cursor.seek_by_key_subkey(1, felt!("0x2").into()).unwrap();
            let val3 = cursor.seek_by_key_subkey(3, felt!("0x3").into()).unwrap();

            let exp_val1 = ContractNonceChange::new(felt!("0x1").into(), felt!("0x2"));
            let exp_val2 = ContractNonceChange::new(felt!("0x2").into(), felt!("0x2"));
            let exp_val3 = ContractNonceChange::new(felt!("0x3").into(), felt!("0x2"));
            assert_eq!(val1, Some(exp_val1));
            assert_eq!(val2, Some(exp_val2));
            assert_eq!(val3, Some(exp_val3));

            let mut cursor = tx.cursor::<tables::ClassChangeHistory>().unwrap();
            let val1 = cursor.seek_by_key_subkey(1, felt!("0x1").into()).unwrap();
            let val2 = cursor.seek_by_key_subkey(1, felt!("0x2").into()).unwrap();

            let exp_val1 = ContractClassChange::new(felt!("0x1").into(), felt!("0x1"));
            let exp_val2 = ContractClassChange::new(felt!("0x2").into(), felt!("0x1"));
            assert_eq!(val1, Some(exp_val1));
            assert_eq!(val2, Some(exp_val2));

            let key1 = ContractStorageKey::new(felt!("0x1").into(), felt!("0x1"));
            let key2 = ContractStorageKey::new(felt!("0x1").into(), felt!("0x2"));
            let key3 = ContractStorageKey::new(felt!("0x2").into(), felt!("0x3"));
            let val1 = tx.get::<tables::StorageChangeSet>(key1).unwrap();
            let val2 = tx.get::<tables::StorageChangeSet>(key2).unwrap();
            let val3 = tx.get::<tables::StorageChangeSet>(key3).unwrap();

            let exp_val1 = BlockList::from([1, 2]);
            let exp_val2 = BlockList::from([1, 3]);
            let exp_val3 = BlockList::from([4, 5]);
            assert_eq!(val1, Some(exp_val1));
            assert_eq!(val2, Some(exp_val2));
            assert_eq!(val3, Some(exp_val3));
        })
        .unwrap();
    }
}
