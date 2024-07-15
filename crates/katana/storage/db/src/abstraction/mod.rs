mod cursor;
mod transaction;

use std::fmt::Debug;

pub use cursor::*;
pub use transaction::*;

use crate::error::DatabaseError;

/// Main persistent database trait. The database implementation must be transactional.
pub trait Database: Send + Sync {
    /// Read-Only transaction
    type Tx: DbTx + Send + Sync + Debug + 'static;
    /// Read-Write transaction
    type TxMut: DbTxMut + Send + Sync + Debug + 'static;

    /// Create and begin read-only transaction.
    #[track_caller]
    fn tx(&self) -> Result<Self::Tx, DatabaseError>;

    /// Create and begin read-write transaction, should return error if the database is unable to
    /// create the transaction e.g, not opened with read-write permission.
    #[track_caller]
    fn tx_mut(&self) -> Result<Self::TxMut, DatabaseError>;

    /// Takes a function and passes a read-only transaction into it, making sure it's closed in the
    /// end of the execution.
    fn view<T, F>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Self::Tx) -> T,
    {
        let tx = self.tx()?;
        let res = f(&tx);
        tx.commit()?;
        Ok(res)
    }

    /// Takes a function and passes a write-read transaction into it, making sure it's committed in
    /// the end of the execution.
    fn update<T, F>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Self::TxMut) -> T,
    {
        let tx = self.tx_mut()?;
        let res = f(&tx);
        tx.commit()?;
        Ok(res)
    }
}
