//! MDBX backend for the database.
//!
//! The code is adapted from `reth` mdbx implementation:  https://github.com/paradigmxyz/reth/blob/227e1b7ad513977f4f48b18041df02686fca5f94/crates/storage/db/src/implementation/mdbx/mod.rs

pub mod cursor;
pub mod tx;

use std::path::Path;

use libmdbx::{DatabaseFlags, EnvironmentFlags, Geometry, Mode, PageSize, SyncMode, RO, RW};

use self::tx::Tx;
use crate::error::DatabaseError;
use crate::tables::{TableType, Tables};
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
#[derive(Debug)]
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

        Ok(DbEnv(builder.open(path.as_ref()).map_err(DatabaseError::OpenEnv)?))
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

    /// Begin a read-only transaction.
    pub fn tx(&self) -> Result<Tx<RO>, DatabaseError> {
        Ok(Tx::new(self.0.begin_ro_txn().map_err(DatabaseError::CreateROTx)?))
    }

    /// Begin a read-write transaction.
    pub fn tx_mut(&self) -> Result<Tx<RW>, DatabaseError> {
        Ok(Tx::new(self.0.begin_rw_txn().map_err(DatabaseError::CreateRWTx)?))
    }
}
