//! MDBX backend for the database.

pub mod cursor;
pub mod models;
pub mod tables;
pub mod tx;

use std::ops::Deref;
use std::path::Path;

use libmdbx::{
    DatabaseFlags, Environment, EnvironmentFlags, EnvironmentKind, Geometry, Mode, PageSize,
    SyncMode, RO, RW,
};

use self::tables::{TableType, Tables};
use self::tx::Tx;
use crate::error::DatabaseError;
use crate::utils;

const GIGABYTE: usize = 1024 * 1024 * 1024;
const TERABYTE: usize = GIGABYTE * 1024;

/// MDBX allows up to 32767 readers (`MDBX_READERS_LIMIT`), but we limit it to slightly below that
const DEFAULT_MAX_READERS: u64 = 32_000;

/// Environment used when opening a MDBX environment. RO/RW.
#[derive(Debug)]
pub enum EnvKind {
    /// Read-only MDBX environment.
    RO,
    /// Read-write MDBX environment.
    RW,
}

/// Wrapper for `libmdbx-sys` environment.
#[derive(Debug)]
pub struct Env<E: EnvironmentKind>(pub libmdbx::Environment<E>);

impl<E: EnvironmentKind> Env<E> {
    /// Opens the database at the specified path with the given `EnvKind`.
    ///
    /// It does not create the tables, for that call [`Env::create_tables`].
    pub fn open(path: &Path, kind: EnvKind) -> Env<E> {
        let mode = match kind {
            EnvKind::RO => Mode::ReadOnly,
            EnvKind::RW => Mode::ReadWrite { sync_mode: SyncMode::Durable },
        };

        let mut inner_env = Environment::new();
        inner_env.set_max_dbs(Tables::ALL.len());
        inner_env.set_geometry(Geometry {
            // Maximum database size of 1 terabytes
            size: Some(0..(1 * TERABYTE)),
            // We grow the database in increments of 4 gigabytes
            growth_step: Some(4 * GIGABYTE as isize),
            // The database never shrinks
            shrink_threshold: None,
            page_size: Some(PageSize::Set(utils::default_page_size())),
        });
        inner_env.set_flags(EnvironmentFlags {
            mode,
            // We disable readahead because it improves performance for linear scans, but
            // worsens it for random access (which is our access pattern outside of sync)
            no_rdahead: true,
            coalesce: true,
            ..Default::default()
        });
        // configure more readers
        inner_env.set_max_readers(DEFAULT_MAX_READERS);

        let env = Env(inner_env.open(path).unwrap());

        env
    }

    /// Creates all the defined tables, if necessary.
    pub fn create_tables(&self) -> Result<(), DatabaseError> {
        let tx = self.begin_rw_txn().map_err(|e| DatabaseError::CreateTransaction(e.into()))?;

        for table in Tables::ALL {
            let flags = match table.table_type() {
                TableType::Table => DatabaseFlags::default(),
                TableType::DupSort => DatabaseFlags::DUP_SORT,
            };

            tx.create_db(Some(table.name()), flags)
                .map_err(|e| DatabaseError::CreateTable(e.into()))?;
        }

        tx.commit().map_err(|e| DatabaseError::Commit(e.into()))?;

        Ok(())
    }
}

impl<'env, E: EnvironmentKind> Env<E> {
    fn tx(&'env self) -> Result<Tx<'env, RO, E>, DatabaseError> {
        Ok(Tx::new(self.0.begin_ro_txn().map_err(DatabaseError::CreateTransaction)?))
    }

    fn tx_mut(&'env self) -> Result<Tx<'env, RW, E>, DatabaseError> {
        Ok(Tx::new(self.0.begin_rw_txn().map_err(DatabaseError::CreateTransaction)?))
    }
}

impl<E: EnvironmentKind> Deref for Env<E> {
    type Target = Environment<E>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
