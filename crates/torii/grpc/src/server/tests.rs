#[cfg(test)]
mod test {
    use anyhow::Result;
    use sqlx::SqlitePool;
    use crate::server::DojoWorld;

    #[sqlx::test(migrations = "../migrations")]
    async fn entities_query_test(pool: SqlitePool) -> Result<()>{

        Ok(())
    }
}