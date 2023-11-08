//! Code adapted from Paradigm's [`reth`](https://github.com/paradigmxyz/reth/tree/main/crates/storage/db) DB implementation.

use std::path::Path;

use anyhow::Result;

pub mod codecs;
pub mod error;
pub mod mdbx;
pub mod utils;

/// Initialize the database at the given path
pub fn init_db<P: AsRef<Path>>(path: P) -> Result<()> {
    Ok(())
}

pub fn open_rw_db<P: AsRef<Path>>(path: P) -> Result<()> {
    Ok(())
}
