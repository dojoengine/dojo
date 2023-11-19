use std::collections::HashMap;
use std::sync::Arc;

use dojo_types::schema::Ty;
use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::error::{Error, QueryError};
use crate::model::{build_sql_model_query, parse_sql_model_members, SqlModelMember};

pub struct SchemaData {
    pub ty: Ty,
    pub sql: String,
}

pub struct ModelCache {
    pool: SqlitePool,
    schemas: RwLock<HashMap<String, Arc<SchemaData>>>,
}

impl ModelCache {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool, schemas: RwLock::new(HashMap::new()) }
    }

    pub async fn schema(&self, model: &str) -> Result<Arc<SchemaData>, Error> {
        {
            let schemas = self.schemas.read().await;
            if let Some(schema_data) = schemas.get(model) {
                return Ok(Arc::clone(schema_data));
            }
        }

        self.update(model).await
    }

    async fn update(&self, model: &str) -> Result<Arc<SchemaData>, Error> {
        let model_members: Vec<SqlModelMember> = sqlx::query_as(
            "SELECT id, model_idx, member_idx, name, type, type_enum, enum_options, key FROM \
             model_members WHERE model_id = ? ORDER BY model_idx ASC, member_idx ASC",
        )
        .bind(model)
        .fetch_all(&self.pool)
        .await?;

        if model_members.is_empty() {
            return Err(QueryError::ModelNotFound(model.into()).into());
        }

        let ty = parse_sql_model_members(model, &model_members);
        let sql = build_sql_model_query(ty.as_struct().unwrap());
        let schema_data = Arc::new(SchemaData { ty, sql });

        let mut schemas = self.schemas.write().await;
        schemas.insert(model.into(), Arc::clone(&schema_data));

        Ok(schema_data)
    }

    pub async fn clear(&self) {
        self.schemas.write().await.clear();
    }
}
