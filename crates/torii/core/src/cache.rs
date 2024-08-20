use std::collections::HashMap;

use dojo_types::schema::Ty;
use sqlx::SqlitePool;
use starknet_crypto::Felt;
use tokio::sync::RwLock;

use crate::error::{Error, QueryError};
use crate::model::{parse_sql_model_members, SqlModelMember};

#[derive(Debug, Clone)]
pub struct Model {
    pub namespace: String,
    pub name: String,
    pub schema: Ty
}

#[derive(Debug)]
pub struct ModelCache {
    pool: SqlitePool,
    cache: RwLock<HashMap<Felt, Model>>,
}

impl ModelCache {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool, cache: RwLock::new(HashMap::new()) }
    }

    pub async fn models(&self, selectors: &[Felt]) -> Result<Vec<Model>, Error> {
        let mut schemas = Vec::with_capacity(selectors.len());
        for selector in selectors {
            schemas.push(self.model(selector).await?);
        }

        Ok(schemas)
    }

    pub async fn model(&self, selector: &Felt) -> Result<Model, Error> {
        {
            let cache = self.cache.read().await;
            if let Some(model) = cache.get(selector).cloned() {
                return Ok(model);
            }
        }

        self.update_model(selector).await
    }

    async fn update_model(&self, selector: &Felt) -> Result<Model, Error> {
        let formatted_selector = format!("{:#x}", selector);

        let (namespace, name): (String, String) =
            sqlx::query_as("SELECT namespace, name FROM models WHERE id = ?")
                .bind(formatted_selector.clone())
                .fetch_one(&self.pool)
                .await?;
        let model_members: Vec<SqlModelMember> = sqlx::query_as(
            "SELECT id, model_idx, member_idx, name, type, type_enum, enum_options, key FROM \
             model_members WHERE model_id = ? ORDER BY model_idx ASC, member_idx ASC",
        )
        .bind(formatted_selector)
        .fetch_all(&self.pool)
        .await?;

        if model_members.is_empty() {
            return Err(QueryError::ModelNotFound(name.clone()).into());
        }

        let schema = parse_sql_model_members(&namespace, &name, &model_members);
        let mut cache = self.cache.write().await;
        
        let model = Model { namespace, name, schema: schema.clone() };
        cache.insert(*selector, model.clone());

        Ok(model)
    }

    pub async fn clear(&self) {
        self.cache.write().await.clear();
    }
}
