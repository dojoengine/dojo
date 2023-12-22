use std::collections::HashMap;

use dojo_types::schema::Ty;
use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::error::{Error, QueryError};
use crate::model::{parse_sql_model_members, SqlModelMember};

type ModelName = String;

pub struct ModelCache {
    pool: SqlitePool,
    cache: RwLock<HashMap<ModelName, Ty>>,
}

impl ModelCache {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool, cache: RwLock::new(HashMap::new()) }
    }

    pub async fn schemas(&self, models: Vec<&str>) -> Result<Vec<Ty>, Error> {
        let mut schemas = Vec::with_capacity(models.len());
        for model in models {
            schemas.push(self.schema(model).await?);
        }

        Ok(schemas)
    }

    pub async fn schema(&self, model: &str) -> Result<Ty, Error> {
        {
            let cache = self.cache.read().await;
            if let Some(schema) = cache.get(model) {
                return Ok(schema.clone());
            }
        }

        self.update_schema(model).await
    }

    async fn update_schema(&self, model: &str) -> Result<Ty, Error> {
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
        let mut cache = self.cache.write().await;
        cache.insert(model.into(), ty.clone());

        Ok(ty)
    }

    pub async fn clear(&self) {
        self.cache.write().await.clear();
    }
}
