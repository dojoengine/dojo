// #[cfg(test)]
// mod tests {
//     use async_graphql_parser::parse_schema;
//     use sqlx::SqlitePool;

//     use crate::graphql::entity::Entity;
//     use crate::graphql::schema::{self, build_schema};
//     use crate::tests::common::run_graphql_query;

//     #[sqlx::test(migrations = "./migrations", fixtures("components"))]
//     async fn test_dynamic_component(pool: SqlitePool) {
//         let schema = build_schema(&pool).await;
//     }
// }
