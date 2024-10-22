//! MDBX backend for the database.
//!
//! The code is adapted from `reth` mdbx implementation:  <https://github.com/paradigmxyz/reth/blob/227e1b7ad513977f4f48b18041df02686fca5f94/crates/storage/db/src/implementation/mdbx/mod.rs>

pub mod cursor;
pub mod stats;
pub mod temp;
pub mod tx;

use std::collections::HashMap;
use std::path::Path;

use dojo_metrics::metrics::gauge;
pub use libmdbx;
use libmdbx::{DatabaseFlags, EnvironmentFlags, Geometry, Mode, PageSize, SyncMode, RO, RW};
use metrics::{describe_gauge, Label};
use tracing::error;

use self::stats::{Stats, TableStat};
use self::tx::Tx;
use crate::abstraction::Database;
use crate::error::DatabaseError;
use crate::tables::{TableType, Tables, NUM_TABLES};
use crate::utils;

const GIGABYTE: usize = 1024 * 1024 * 1024;
const TERABYTE: usize = GIGABYTE * 1024;

/// MDBX allows up to 32767 readers (`MDBX_READERS_LIMIT`), but we limit it to slightly below that
const DEFAULT_MAX_READERS: u64 = 32_000;

/// Environment used when opening a MDBX environment. RO/RW.
#[derive(Debug)]
pub enum DbEnvKind {
    /// Read-only MDBX environment.
    RO,
    /// Read-write MDBX environment.
    RW,
}

/// Wrapper for `libmdbx-sys` environment.
#[derive(Debug, Clone)]
pub struct DbEnv(libmdbx::Environment);

impl DbEnv {
    /// Opens the database at the specified path with the given `EnvKind`.
    ///
    /// It does not create the tables, for that call [`DbEnv::create_tables`].
    pub fn open(path: impl AsRef<Path>, kind: DbEnvKind) -> Result<DbEnv, DatabaseError> {
        let mode = match kind {
            DbEnvKind::RO => Mode::ReadOnly,
            DbEnvKind::RW => Mode::ReadWrite { sync_mode: SyncMode::Durable },
        };

        let mut builder = libmdbx::Environment::builder();
        builder
            .set_max_dbs(Tables::ALL.len())
            .set_geometry(Geometry {
                // Maximum database size of 1 terabytes
                size: Some(0..(TERABYTE)),
                // We grow the database in increments of 4 gigabytes
                growth_step: Some(4 * GIGABYTE as isize),
                // The database never shrinks
                shrink_threshold: None,
                page_size: Some(PageSize::Set(utils::default_page_size())),
            })
            .set_flags(EnvironmentFlags {
                mode,
                // We disable readahead because it improves performance for linear scans, but
                // worsens it for random access (which is our access pattern outside of sync)
                no_rdahead: true,
                coalesce: true,
                ..Default::default()
            })
            .set_max_readers(DEFAULT_MAX_READERS);

        Ok(DbEnv(builder.open(path.as_ref()).map_err(DatabaseError::OpenEnv)?).with_metrics())
    }

    /// Creates all the defined tables in [`Tables`], if necessary.
    pub fn create_tables(&self) -> Result<(), DatabaseError> {
        let tx = self.0.begin_rw_txn().map_err(DatabaseError::CreateRWTx)?;

        for table in Tables::ALL {
            let flags = match table.table_type() {
                TableType::Table => DatabaseFlags::default(),
                TableType::DupSort => DatabaseFlags::DUP_SORT,
            };

            tx.create_db(Some(table.name()), flags).map_err(DatabaseError::CreateTable)?;
        }

        tx.commit().map_err(DatabaseError::Commit)?;

        Ok(())
    }

    fn with_metrics(self) -> Self {
        describe_gauge!("db.table_size", metrics::Unit::Bytes, "Total size of the table");
        describe_gauge!("db.table_pages", metrics::Unit::Count, "Number of pages in the table");
        describe_gauge!("db.table_entries", metrics::Unit::Count, "Number of entries in the table");
        describe_gauge!("db.freelist", metrics::Unit::Bytes, "Size of the database freelist");
        self
    }
}

impl Database for DbEnv {
    type Tx = tx::Tx<RO>;
    type TxMut = tx::Tx<RW>;
    type Stats = stats::Stats;

    fn tx(&self) -> Result<Self::Tx, DatabaseError> {
        Ok(Tx::new(self.0.begin_ro_txn().map_err(DatabaseError::CreateROTx)?))
    }

    fn tx_mut(&self) -> Result<Self::TxMut, DatabaseError> {
        Ok(Tx::new(self.0.begin_rw_txn().map_err(DatabaseError::CreateRWTx)?))
    }

    fn stats(&self) -> Result<Self::Stats, DatabaseError> {
        self.view(|tx| {
            let mut table_stats = HashMap::with_capacity(NUM_TABLES);

            for table in Tables::ALL.iter() {
                let dbi = tx.inner.open_db(Some(table.name())).map_err(DatabaseError::OpenDb)?;
                let stat = tx.inner.db_stat(&dbi).map_err(DatabaseError::GetStats)?;
                table_stats.insert(table.name(), TableStat::new(stat));
            }

            let info = self.0.info().map_err(DatabaseError::Stat)?;
            let freelist = self.0.freelist().map_err(DatabaseError::Stat)?;
            Ok(Stats { table_stats, info, freelist })
        })?
    }
}

impl dojo_metrics::Report for DbEnv {
    fn report(&self) {
        match self.stats() {
            Ok(stats) => {
                let mut pgsize = 0;

                for (table, stat) in stats.table_stats() {
                    gauge!("db.table_size", vec![Label::new("table", *table)])
                        .set(stat.total_size() as f64);

                    gauge!(
                        "db.table_pages",
                        vec![Label::new("table", *table), Label::new("type", "leaf")]
                    )
                    .set(stat.leaf_pages() as f64);

                    gauge!(
                        "db.table_pages",
                        vec![Label::new("table", *table), Label::new("type", "branch")]
                    )
                    .set(stat.branch_pages() as f64);

                    gauge!(
                        "db.table_pages",
                        vec![Label::new("table", *table), Label::new("type", "overflow")]
                    )
                    .set(stat.overflow_pages() as f64);

                    gauge!("db.table_entries", vec![Label::new("table", *table)])
                        .set(stat.entries() as f64);

                    if pgsize == 0 {
                        pgsize = stat.page_size() as usize;
                    }
                }

                gauge!("db.freelist").set((stats.freelist() * pgsize) as f64);
            }

            Err(error) => {
                error!(%error, "Failed to read database stats.");
            }
        }
    }
}

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils {
    use std::path::Path;

    use super::{DbEnv, DbEnvKind};

    const ERROR_DB_CREATION: &str = "Not able to create the mdbx file.";

    /// Create database for testing
    pub fn create_test_db(kind: DbEnvKind) -> DbEnv {
        create_test_db_with_path(
            kind,
            &tempfile::TempDir::new().expect("Failed to create temp dir.").into_path(),
        )
    }

    /// Create database for testing with specified path
    pub fn create_test_db_with_path(kind: DbEnvKind, path: &Path) -> DbEnv {
        let env = DbEnv::open(path, kind).expect(ERROR_DB_CREATION);
        env.create_tables().expect("Failed to create tables.");
        env
    }
}

#[cfg(test)]
mod tests {

    use katana_primitives::block::Header;
    use katana_primitives::contract::{ContractAddress, GenericContractInfo};
    use katana_primitives::{address, Felt};
    use starknet::macros::felt;

    use super::*;
    use crate::abstraction::{DbCursor, DbCursorMut, DbDupSortCursor, DbTx, DbTxMut, Walker};
    use crate::codecs::Encode;
    use crate::mdbx::test_utils::create_test_db;
    use crate::models::storage::StorageEntry;
    use crate::tables::{BlockHashes, ContractInfo, ContractStorage, Headers, Table};

    const ERROR_PUT: &str = "Not able to insert value into table.";
    const ERROR_DELETE: &str = "Failed to delete value from table.";
    const ERROR_GET: &str = "Not able to get value from table.";
    const ERROR_COMMIT: &str = "Not able to commit transaction.";
    const ERROR_RETURN_VALUE: &str = "Mismatching result.";
    const ERROR_UPSERT: &str = "Not able to upsert the value to the table.";
    const ERROR_INIT_TX: &str = "Failed to create a MDBX transaction.";
    const ERROR_INIT_CURSOR: &str = "Failed to create cursor.";
    const ERROR_GET_AT_CURSOR_POS: &str = "Failed to get value at cursor position.";

    #[test]
    fn db_creation() {
        create_test_db(DbEnvKind::RW);
    }

    #[test]
    fn db_stats() {
        let env = create_test_db(DbEnvKind::RW);

        // Insert some data to ensure non-zero stats
        let tx = env.tx_mut().expect(ERROR_INIT_TX);
        tx.put::<Headers>(1u64, Header::default()).expect(ERROR_PUT);
        tx.commit().expect(ERROR_COMMIT);

        // Retrieve stats
        let stats = env.stats().expect("Failed to retrieve database stats");

        // Check overall stats
        assert!(stats.total_entries() > 0, "Total entries should be non-zero");
        assert!(stats.total_pages() > 0, "Total pages should be non-zero");
        assert!(stats.map_size() > 0, "Map size should be non-zero");

        // Check table-specific stats
        let headers_stat = stats.table_stat(Headers::NAME).expect("Headers table stats not found");
        assert!(headers_stat.entries() > 0, "Headers table should have entries");
        assert!(headers_stat.leaf_pages() > 0, "Headers table should have leaf pages");

        // Verify that we can access stats for all tables
        for table in Tables::ALL {
            assert!(
                stats.table_stat(table.name()).is_some(),
                "Stats for table {} not found",
                table.name()
            );
        }
    }

    #[test]
    fn db_manual_put_get() {
        let env = create_test_db(DbEnvKind::RW);

        let value = Header::default();
        let key = 1u64;

        // PUT
        let tx = env.tx_mut().expect(ERROR_INIT_TX);
        tx.put::<Headers>(key, value.clone()).expect(ERROR_PUT);
        tx.commit().expect(ERROR_COMMIT);

        // GET
        let tx = env.tx().expect(ERROR_INIT_TX);
        let result = tx.get::<Headers>(key).expect(ERROR_GET);
        let total_entries = tx.entries::<Headers>().expect(ERROR_GET);
        tx.commit().expect(ERROR_COMMIT);

        assert!(total_entries == 1);
        assert!(result.expect(ERROR_RETURN_VALUE) == value);
    }

    #[test]
    fn db_delete() {
        let env = create_test_db(DbEnvKind::RW);

        let value = Header::default();
        let key = 1u64;

        // PUT
        let tx = env.tx_mut().expect(ERROR_INIT_TX);
        tx.put::<Headers>(key, value).expect(ERROR_PUT);
        tx.commit().expect(ERROR_COMMIT);

        let entries = env.tx().expect(ERROR_INIT_TX).entries::<Headers>().expect(ERROR_GET);
        assert!(entries == 1);

        // DELETE
        let tx = env.tx_mut().expect(ERROR_INIT_TX);
        tx.delete::<Headers>(key, None).expect(ERROR_DELETE);
        tx.commit().expect(ERROR_COMMIT);

        let entries = env.tx().expect(ERROR_INIT_TX).entries::<Headers>().expect(ERROR_GET);
        assert!(entries == 0);
    }

    #[test]
    fn db_manual_cursor_walk() {
        let env = create_test_db(DbEnvKind::RW);

        let key1 = 1u64;
        let key2 = 2u64;
        let key3 = 3u64;
        let header1 = Header::default();
        let header2 = Header::default();
        let header3 = Header::default();

        // PUT
        let tx = env.tx_mut().expect(ERROR_INIT_TX);
        tx.put::<Headers>(key1, header1.clone()).expect(ERROR_PUT);
        tx.put::<Headers>(key2, header2.clone()).expect(ERROR_PUT);
        tx.put::<Headers>(key3, header3.clone()).expect(ERROR_PUT);
        tx.commit().expect(ERROR_COMMIT);

        // CURSOR
        let tx = env.tx().expect(ERROR_INIT_TX);
        let mut cursor = tx.cursor::<Headers>().expect(ERROR_INIT_CURSOR);
        let (_, result1) = cursor.next().expect(ERROR_GET_AT_CURSOR_POS).expect(ERROR_RETURN_VALUE);
        let (_, result2) = cursor.next().expect(ERROR_GET_AT_CURSOR_POS).expect(ERROR_RETURN_VALUE);
        let (_, result3) = cursor.next().expect(ERROR_GET_AT_CURSOR_POS).expect(ERROR_RETURN_VALUE);
        tx.commit().expect(ERROR_COMMIT);

        assert!(result1 == header1);
        assert!(result2 == header2);
        assert!(result3 == header3);
    }

    #[test]
    fn db_cursor_upsert() {
        let db = create_test_db(DbEnvKind::RW);
        let tx = db.tx_mut().expect(ERROR_INIT_TX);

        let mut cursor = tx.cursor::<ContractInfo>().unwrap();
        let key = address!("0x1337");

        let account = GenericContractInfo::default();
        cursor.upsert(key, account).expect(ERROR_UPSERT);
        assert_eq!(cursor.set(key), Ok(Some((key, account))));

        let account = GenericContractInfo { nonce: 1u8.into(), ..Default::default() };
        cursor.upsert(key, account).expect(ERROR_UPSERT);
        assert_eq!(cursor.set(key), Ok(Some((key, account))));

        let account = GenericContractInfo { nonce: 1u8.into(), ..Default::default() };
        cursor.upsert(key, account).expect(ERROR_UPSERT);
        assert_eq!(cursor.set(key), Ok(Some((key, account))));

        let mut dup_cursor = tx.cursor::<ContractStorage>().unwrap();
        let subkey = felt!("0x9");

        let value = Felt::from(1u8);
        let entry1 = StorageEntry { key: subkey, value };
        dup_cursor.upsert(key, entry1).expect(ERROR_UPSERT);
        assert_eq!(dup_cursor.seek_by_key_subkey(key, subkey), Ok(Some(entry1)));

        let value = Felt::from(2u8);
        let entry2 = StorageEntry { key: subkey, value };
        dup_cursor.upsert(key, entry2).expect(ERROR_UPSERT);
        assert_eq!(dup_cursor.seek_by_key_subkey(key, subkey), Ok(Some(entry1)));
        assert_eq!(dup_cursor.next_dup_val(), Ok(Some(entry2)));
    }

    #[test]
    fn db_cursor_walk() {
        let env = create_test_db(DbEnvKind::RW);

        let value = Header::default();
        let key = 1u64;

        // PUT
        let tx = env.tx_mut().expect(ERROR_INIT_TX);
        tx.put::<Headers>(key, value.clone()).expect(ERROR_PUT);
        tx.commit().expect(ERROR_COMMIT);

        // Cursor
        let tx = env.tx().expect(ERROR_INIT_TX);
        let mut cursor = tx.cursor::<Headers>().expect(ERROR_INIT_CURSOR);

        let first = cursor.first().unwrap();
        assert!(first.is_some(), "First should be our put");

        // Walk
        let walk = cursor.walk(Some(key)).unwrap();
        let first = walk.into_iter().next().unwrap().unwrap();
        assert_eq!(first.1, value, "First next should be put value");
    }

    #[test]
    fn db_walker() {
        let db = create_test_db(DbEnvKind::RW);

        // PUT (0, 0), (1, 0), (2, 0)
        let tx = db.tx_mut().expect(ERROR_INIT_TX);
        (0..3).try_for_each(|key| tx.put::<BlockHashes>(key, Felt::ZERO)).expect(ERROR_PUT);
        tx.commit().expect(ERROR_COMMIT);

        let tx = db.tx().expect(ERROR_INIT_TX);
        let mut cursor = tx.cursor::<BlockHashes>().expect(ERROR_INIT_CURSOR);
        let mut walker = Walker::new(&mut cursor, None);

        assert_eq!(walker.next(), Some(Ok((0, Felt::ZERO))));
        assert_eq!(walker.next(), Some(Ok((1, Felt::ZERO))));
        assert_eq!(walker.next(), Some(Ok((2, Felt::ZERO))));
        assert_eq!(walker.next(), None);
    }

    #[test]
    fn db_cursor_insert() {
        let db = create_test_db(DbEnvKind::RW);

        // PUT
        let tx = db.tx_mut().expect(ERROR_INIT_TX);
        (0..=4).try_for_each(|key| tx.put::<BlockHashes>(key, Felt::ZERO)).expect(ERROR_PUT);
        tx.commit().expect(ERROR_COMMIT);

        let key_to_insert = 5;
        let tx = db.tx_mut().expect(ERROR_INIT_TX);
        let mut cursor = tx.cursor::<BlockHashes>().expect(ERROR_INIT_CURSOR);

        // INSERT
        assert_eq!(cursor.insert(key_to_insert, Felt::ZERO), Ok(()));
        assert_eq!(cursor.current(), Ok(Some((key_to_insert, Felt::ZERO))));

        // INSERT (failure)
        assert_eq!(
            cursor.insert(key_to_insert, Felt::ZERO),
            Err(DatabaseError::Write {
                table: BlockHashes::NAME,
                error: libmdbx::Error::KeyExist,
                key: Box::from(key_to_insert.encode())
            })
        );
        assert_eq!(cursor.current(), Ok(Some((key_to_insert, Felt::ZERO))));

        tx.commit().expect(ERROR_COMMIT);

        // Confirm the result
        let tx = db.tx().expect(ERROR_INIT_TX);
        let mut cursor = tx.cursor::<BlockHashes>().expect(ERROR_INIT_CURSOR);
        let res = cursor.walk(None).unwrap().map(|res| res.unwrap().0).collect::<Vec<_>>();
        assert_eq!(res, vec![0, 1, 2, 3, 4, 5]);
        tx.commit().expect(ERROR_COMMIT);
    }

    #[test]
    fn db_dup_sort() {
        let env = create_test_db(DbEnvKind::RW);
        let key = address!("0xa2c122be93b0074270ebee7f6b7292c7deb45047");

        // PUT (0,0)
        let value00 = StorageEntry::default();
        env.update(|tx| tx.put::<ContractStorage>(key, value00).expect(ERROR_PUT)).unwrap();

        // PUT (2,2)
        let value22 = StorageEntry { key: felt!("2"), value: felt!("2") };
        env.update(|tx| tx.put::<ContractStorage>(key, value22).expect(ERROR_PUT)).unwrap();

        // // PUT (1,1)
        let value11 = StorageEntry { key: felt!("1"), value: felt!("1") };
        env.update(|tx| tx.put::<ContractStorage>(key, value11).expect(ERROR_PUT)).unwrap();

        // Iterate with cursor
        {
            let tx = env.tx().expect(ERROR_INIT_TX);
            let mut cursor = tx.cursor::<ContractStorage>().unwrap();

            // Notice that value11 and value22 have been ordered in the DB.
            assert!(Some(value00) == cursor.next_dup_val().unwrap());
            assert!(Some(value11) == cursor.next_dup_val().unwrap());
            assert!(Some(value22) == cursor.next_dup_val().unwrap());
        }

        // Seek value with exact subkey
        {
            let tx = env.tx().expect(ERROR_INIT_TX);
            let mut cursor = tx.cursor::<ContractStorage>().unwrap();
            let mut walker = cursor.walk_dup(Some(key), Some(felt!("1"))).unwrap().unwrap();

            assert_eq!(
                (key, value11),
                walker
                    .next()
                    .expect("element should exist.")
                    .expect("should be able to retrieve it.")
            );
        }
    }
}
