#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error(transparent)]
    Codec(#[from] CodecError),

    #[error("failed to create table: {0}")]
    CreateTable(libmdbx::Error),

    #[error("failed to commit transaction: {0}")]
    Commit(libmdbx::Error),

    #[error("failed to read: {0}")]
    Read(libmdbx::Error),

    #[error("failed to write to table {table} with key {key:?}: {error}")]
    Write { error: libmdbx::Error, table: &'static str, key: Box<[u8]> },

    #[error("failed to open database: {0}")]
    OpenDb(libmdbx::Error),

    #[error("failed to retrieve database statistics: {0}")]
    Stat(libmdbx::Error),

    #[error("failed to create cursor: {0}")]
    CreateCursor(libmdbx::Error),

    #[error("failed to create transaction: {0}")]
    CreateTransaction(libmdbx::Error),

    #[error("failed to delete entry: {0}")]
    Delete(libmdbx::Error),

    #[error("failed to clear database: {0}")]
    Clear(libmdbx::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("failed to decode data: {0}")]
    Decode(String),

    #[error("failed to decompress data: {0}")]
    Decompress(String),
}
