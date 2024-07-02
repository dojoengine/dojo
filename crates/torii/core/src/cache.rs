use std::collections::HashMap;

use dojo_types::schema::Model;
use sqlx::SqlitePool;
use starknet_crypto::FieldElement;
use tokio::sync::RwLock;

use crate::error::{Error, QueryError};
use crate::model::{parse_sql_model_members, SqlModelMember};

pub struct ModelCache {
    pool: SqlitePool,
    cache: RwLock<HashMap<FieldElement, Model>>,
}

impl ModelCache {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool, cache: RwLock::new(HashMap::new()) }
    }

    pub async fn schemas(&self, selectors: &[FieldElement]) -> Result<Vec<Model>, Error> {
        let mut schemas = Vec::with_capacity(selectors.len());
        for selector in selectors {
            schemas.push(self.schema(&selector).await?);
        }

        Ok(schemas)
    }

    pub async fn schema(&self, selector: &FieldElement) -> Result<Model, Error> {
        {
            let cache = self.cache.read().await;
            if let Some(model) = cache.get(selector).cloned() {
                return Ok(model);
            }
        }

        self.update_schema(selector).await
    }

    async fn update_schema(&self, selector: &FieldElement) -> Result<Model, Error> {
        let formatted_selector = format!("{:#x}", selector);

        let (namespace, name): (String, String) =
            sqlx::query_as("SELECT namespace, name FROM models WHERE id = ?")
                .bind(formatted_selector.clone())
                .fetch_one(&self.pool)
                .await?;
        let model_members: Vec<SqlModelMember> = sqlx::query_as(
            "SELECT id, model_idx, member_idx, namespace, name, type, type_enum, enum_options, key FROM \
             model_members WHERE model_id = ? ORDER BY model_idx ASC, member_idx ASC",
        )
        .bind(formatted_selector)
        .fetch_all(&self.pool)
        .await?;

        if model_members.is_empty() {
            return Err(QueryError::ModelNotFound(name.clone()).into());
        }

        let model = Model {
            namespace,
            name: name.clone(),
            members: parse_sql_model_members(&name, &model_members)
                .as_struct()
                .unwrap()
                .children
                .clone(),
        };
        let mut cache = self.cache.write().await;
        cache.insert(selector.clone(), model.clone());

        Ok(model)
    }

    pub async fn clear(&self) {
        self.cache.write().await.clear();
    }
}
