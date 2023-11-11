//! Code adapted from Paradigm's [`reth`](https://github.com/paradigmxyz/reth/tree/main/crates/storage/db) DB implementation.

use std::path::Path;

use anyhow::Context;
use libmdbx::WriteMap;

pub mod codecs;
pub mod error;
pub mod mdbx;
pub mod utils;

use mdbx::{Env, EnvKind};
use utils::is_database_empty;

/// Initialize the database at the given path and returning a handle to the its
/// environment.
///
/// This will create the default tables, if necessary.
pub fn init_db<P: AsRef<Path>>(path: P) -> anyhow::Result<Env<WriteMap>> {
    if is_database_empty(path.as_ref()) {
        // TODO: create dir if it doesn't exist and insert db version file
        std::fs::create_dir_all(path.as_ref()).with_context(|| {
            format!("Creating database directory at path {}", path.as_ref().display())
        })?;
    } else {
        // TODO: check if db version file exists and if it's compatible
    }
    let env = open_db(path)?;
    env.create_tables()?;
    Ok(env)
}

/// Open the database at the given `path` in read-write mode.
pub fn open_db<P: AsRef<Path>>(path: P) -> anyhow::Result<Env<WriteMap>> {
    Env::open(path.as_ref(), EnvKind::RW).with_context(|| {
        format!("Opening database in read-write mode at path {}", path.as_ref().display())
    })
}
