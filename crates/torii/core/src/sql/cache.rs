use std::collections::{HashMap, HashSet};

use dojo_types::schema::Ty;
use dojo_world::contracts::abi::model::Layout;
use sqlx::{Pool, Sqlite, SqlitePool};
use starknet_crypto::Felt;
use tokio::sync::RwLock;

use crate::error::{Error, ParseError, QueryError};
use crate::model::{parse_sql_model_members, SqlModelMember};
use crate::sql::utils::I256;
use crate::types::ContractType;

#[derive(Debug, Clone)]
pub struct Model {
    /// Namespace of the model
    pub namespace: String,
    /// The name of the model
    pub name: String,
    /// The selector of the model
    pub selector: Felt,
    /// The class hash of the model
    pub class_hash: Felt,
    /// The contract address of the model
    pub contract_address: Felt,
    pub packed_size: u32,
    pub unpacked_size: u32,
    pub layout: Layout,
    pub schema: Ty,
}

#[derive(Debug)]
pub struct ModelCache {
    pool: SqlitePool,
    model_cache: RwLock<HashMap<Felt, Model>>,
}

impl ModelCache {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool, model_cache: RwLock::new(HashMap::new()) }
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
            let cache = self.model_cache.read().await;
            if let Some(model) = cache.get(selector).cloned() {
                return Ok(model);
            }
        }

        self.update_model(selector).await
    }

    async fn update_model(&self, selector: &Felt) -> Result<Model, Error> {
        let formatted_selector = format!("{:#x}", selector);

        let (namespace, name, class_hash, contract_address, packed_size, unpacked_size, layout): (
            String,
            String,
            String,
            String,
            u32,
            u32,
            String,
        ) = sqlx::query_as(
            "SELECT namespace, name, class_hash, contract_address, packed_size, unpacked_size, \
             layout FROM models WHERE id = ?",
        )
        .bind(format!("{:#x}", selector))
        .fetch_one(&self.pool)
        .await?;

        let class_hash = Felt::from_hex(&class_hash).map_err(ParseError::FromStr)?;
        let contract_address = Felt::from_hex(&contract_address).map_err(ParseError::FromStr)?;

        let layout = serde_json::from_str(&layout).map_err(ParseError::FromJsonStr)?;

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
        let mut cache = self.model_cache.write().await;

        let model = Model {
            namespace,
            name,
            selector: *selector,
            class_hash,
            contract_address,
            packed_size,
            unpacked_size,
            layout,
            schema,
        };
        cache.insert(*selector, model.clone());

        Ok(model)
    }

    pub async fn set(&self, selector: Felt, model: Model) {
        let mut cache = self.model_cache.write().await;
        cache.insert(selector, model);
    }

    pub async fn clear(&self) {
        self.model_cache.write().await.clear();
    }
}

#[derive(Debug)]
pub struct LocalCache {
    pub erc_cache: HashMap<(ContractType, String), I256>,
    pub token_id_registry: HashSet<String>,
}

impl Clone for LocalCache {
    fn clone(&self) -> Self {
        Self { erc_cache: HashMap::new(), token_id_registry: HashSet::new() }
    }
}

impl LocalCache {
    pub async fn new(pool: Pool<Sqlite>) -> Self {
        // read existing token_id's from balances table and cache them
        let token_id_registry: Vec<(String,)> = sqlx::query_as("SELECT token_id FROM balances")
            .fetch_all(&pool)
            .await
            .expect("Should be able to read token_id's from blances table");

        let token_id_registry = token_id_registry.into_iter().map(|token_id| token_id.0).collect();

        Self { erc_cache: HashMap::new(), token_id_registry }
    }

    pub fn empty() -> Self {
        Self { erc_cache: HashMap::new(), token_id_registry: HashSet::new() }
    }

    pub fn contains_token_id(&self, token_id: &str) -> bool {
        self.token_id_registry.contains(token_id)
    }

    pub fn register_token_id(&mut self, token_id: String) {
        self.token_id_registry.insert(token_id);
    }
}
