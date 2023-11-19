use std::collections::HashMap;

use dojo_types::schema::Ty;
use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::error::{Error, QueryError};
use crate::model::{build_sql_model_query, parse_sql_model_members, SqlModelMember};

#[derive(Clone)]
pub struct SchemaInfo {
    pub ty: Ty,
    pub query: String,
}

pub struct ModelCache {
    pool: SqlitePool,
    schemas: RwLock<HashMap<String, SchemaInfo>>,
}

impl ModelCache {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool, schemas: RwLock::new(HashMap::new()) }
    }

    pub async fn schema(&self, model: &str) -> Result<SchemaInfo, Error> {
        {
            let schemas = self.schemas.read().await;
            if let Some(schema_info) = schemas.get(model) {
                return Ok(schema_info.clone());
            }
        }

        self.update(model).await
    }

    async fn update(&self, model: &str) -> Result<SchemaInfo, Error> {
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
        let query = build_sql_model_query(ty.as_struct().unwrap());
        let schema_info = SchemaInfo { ty, query };

        let mut schemas = self.schemas.write().await;
        schemas.insert(model.into(), schema_info.clone());

        Ok(schema_info)
    }

    pub async fn clear(&self) {
        self.schemas.write().await.clear();
    }
}
