#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum DatabaseError {
    #[error("failed to open db environment: {0}")]
    OpenEnv(libmdbx::Error),

    #[error(transparent)]
    Codec(#[from] CodecError),

    #[error("failed to create db table: {0}")]
    CreateTable(libmdbx::Error),

    #[error("failed to commit db transaction: {0}")]
    Commit(libmdbx::Error),

    #[error("failed to read db: {0}")]
    Read(libmdbx::Error),

    #[error("failed to write to db table {table} with key {key:?}: {error}")]
    Write { error: libmdbx::Error, table: &'static str, key: Box<[u8]> },

    #[error("failed to open db: {0}")]
    OpenDb(libmdbx::Error),

    #[error("failed to retrieve db statistics: {0}")]
    Stat(libmdbx::Error),

    #[error("failed to create db cursor: {0}")]
    CreateCursor(libmdbx::Error),

    #[error("failed to create read-only db transaction: {0}")]
    CreateROTx(libmdbx::Error),

    #[error("failed to create a read-write db transaction: {0}")]
    CreateRWTx(libmdbx::Error),

    #[error("failed to delete a db entry: {0}")]
    Delete(libmdbx::Error),

    #[error("failed to clear db: {0}")]
    Clear(libmdbx::Error),
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum CodecError {
    #[error("failed to decode data: {0}")]
    Decode(String),

    #[error("failed to decompress data: {0}")]
    Decompress(String),
}
