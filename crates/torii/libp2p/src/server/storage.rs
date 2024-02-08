use sqlx::{Pool, Sqlite};

pub(crate) struct RelayStorage {
    pub(crate) pool: Pool<Sqlite>,
}

impl RelayStorage {
    pub(crate) fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

