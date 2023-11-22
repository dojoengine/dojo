use std::collections::HashMap;

use dojo_types::schema::Ty;
use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::error::{Error, QueryError};
use crate::model::{parse_sql_model_members, SqlModelMember};

type EntityId = String;
type ModelName = String;

pub struct ModelCache {
    pool: SqlitePool,
    models: RwLock<HashMap<EntityId, Vec<ModelName>>>,
    schemas: RwLock<HashMap<ModelName, Ty>>,
}

impl ModelCache {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool, models: RwLock::new(HashMap::new()), schemas: RwLock::new(HashMap::new()) }
    }

    pub async fn model(&self, entity_id: &str) -> Result<Vec<ModelName>, Error> {
        {
            let models = self.models.read().await;
            if let Some(models) = models.get(entity_id) {
                return Ok(models.clone());
            }
        }

        self.update_model(entity_id).await
    }

    pub async fn schema(&self, model: &str) -> Result<Ty, Error> {
        {
            let schemas = self.schemas.read().await;
            if let Some(schema) = schemas.get(model) {
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
        let mut schemas = self.schemas.write().await;
        schemas.insert(model.into(), ty.clone());

        Ok(ty)
    }

    async fn update_model(&self, entity_id: &str) -> Result<Vec<ModelName>, Error> {
        let (model_names,): (String,) =
            sqlx::query_as("SELECT model_names FROM entities WHERE id = ?")
                .bind(entity_id)
                .fetch_one(&self.pool)
                .await?;

        Ok(model_names.split(",").map(|s| s.to_string()).collect())
    }

    pub async fn clear(&self) {
        self.schemas.write().await.clear();
    }
}
