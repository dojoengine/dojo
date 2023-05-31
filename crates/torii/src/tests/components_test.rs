#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    use crate::graphql::schema::build_schema;

    #[sqlx::test(migrations = "./migrations", fixtures("components"))]
    async fn test_dynamic_component(pool: SqlitePool) {
        let schema = build_schema(&pool).await.expect("failed to build schema");
    }
}
