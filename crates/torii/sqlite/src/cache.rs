use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use dojo_types::schema::Ty;
use dojo_world::contracts::abigen::model::Layout;
use sqlx::{Pool, Sqlite, SqlitePool};
use starknet::core::types::contract::AbiEntry;
use starknet::core::types::{
    BlockId, BlockTag, ContractClass, EntryPointsByType, LegacyContractAbiEntry, StarknetError,
};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::{Provider, ProviderError};
use starknet_crypto::Felt;
use tokio::sync::{Mutex, RwLock};

use crate::constants::TOKEN_BALANCE_TABLE;
use crate::error::{Error, ParseError};
use crate::types::ContractType;
use crate::utils::I256;

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
        if selectors.is_empty() {
            return Ok(self.model_cache.read().await.values().cloned().collect());
        }

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
        let (
            namespace,
            name,
            class_hash,
            contract_address,
            packed_size,
            unpacked_size,
            layout,
            schema,
        ): (String, String, String, String, u32, u32, String, String) = sqlx::query_as(
            "SELECT namespace, name, class_hash, contract_address, packed_size, unpacked_size, \
             layout, schema FROM models WHERE id = ?",
        )
        .bind(format!("{:#x}", selector))
        .fetch_one(&self.pool)
        .await?;

        let class_hash = Felt::from_hex(&class_hash).map_err(ParseError::FromStr)?;
        let contract_address = Felt::from_hex(&contract_address).map_err(ParseError::FromStr)?;

        let layout = serde_json::from_str(&layout).map_err(ParseError::FromJsonStr)?;
        let schema = serde_json::from_str(&schema).map_err(ParseError::FromJsonStr)?;

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
    pub erc_cache: RwLock<HashMap<(ContractType, String), I256>>,
    // we are using a mutex here because this needs to be atomic.
    // since we paralellize token transfers and thus token reigstrations
    pub token_id_registry: Mutex<HashSet<String>>,
}

impl LocalCache {
    pub async fn new(pool: Pool<Sqlite>) -> Self {
        // read existing token_id's from balances table and cache them
        let token_id_registry: Vec<(String,)> =
            sqlx::query_as(&format!("SELECT token_id FROM {TOKEN_BALANCE_TABLE}"))
                .fetch_all(&pool)
                .await
                .expect("Should be able to read token_id's from blances table");

        let token_id_registry = token_id_registry.into_iter().map(|token_id| token_id.0).collect();

        Self {
            erc_cache: RwLock::new(HashMap::new()),
            token_id_registry: Mutex::new(token_id_registry),
        }
    }

    pub async fn contains_token_id(&self, token_id: &str) -> bool {
        self.token_id_registry.lock().await.contains(token_id)
    }

    pub async fn register_token_id(&self, token_id: String) {
        self.token_id_registry.lock().await.insert(token_id);
    }

    pub async fn try_register_token_id(&self, token_id: String) -> bool {
        let mut registry = self.token_id_registry.lock().await;
        if registry.contains(&token_id) {
            return false;
        }

        registry.insert(token_id);
        true
    }
}

#[derive(Debug, Clone)]
pub enum ClassAbi {
    Sierra((EntryPointsByType, Vec<AbiEntry>)),
    Legacy(Vec<LegacyContractAbiEntry>),
}

#[derive(Debug)]
pub struct ContractClassCache<P: Provider + Sync + std::fmt::Debug> {
    pub classes: RwLock<HashMap<Felt, (Felt, ClassAbi)>>,
    pub provider: Arc<P>,
}

impl<P: Provider + Sync + std::fmt::Debug> ContractClassCache<P> {
    pub fn new(provider: Arc<P>) -> Self {
        Self { classes: RwLock::new(HashMap::new()), provider }
    }

    pub async fn get(
        &self,
        contract_address: Felt,
        mut block_id: BlockId,
    ) -> Result<ClassAbi, Error> {
        {
            let classes = self.classes.read().await;
            if let Some(class) = classes.get(&contract_address) {
                return Ok(class.1.clone());
            }
        }

        let class_hash = match self.provider.get_class_hash_at(block_id, contract_address).await {
            Ok(class_hash) => class_hash,
            Err(e) => match e {
                // if we got a block not found error, we probably are in a pending block.
                ProviderError::StarknetError(StarknetError::BlockNotFound) => {
                    block_id = BlockId::Tag(BlockTag::Pending);
                    self.provider.get_class_hash_at(block_id, contract_address).await?
                }
                _ => return Err(Error::ProviderError(e)),
            },
        };
        let class = match self.provider.get_class_at(block_id, contract_address).await? {
            ContractClass::Sierra(sierra) => {
                let abi: Vec<AbiEntry> = serde_json::from_str(&sierra.abi).unwrap();
                let functions: Vec<AbiEntry> = flatten_abi_funcs_recursive(&abi);
                ClassAbi::Sierra((sierra.entry_points_by_type, functions))
            }
            ContractClass::Legacy(legacy) => ClassAbi::Legacy(legacy.abi.unwrap_or_default()),
        };
        self.classes.write().await.insert(contract_address, (class_hash, class.clone()));
        Ok(class)
    }
}

fn flatten_abi_funcs_recursive(abi: &[AbiEntry]) -> Vec<AbiEntry> {
    abi.iter()
        .flat_map(|entry| match entry {
            AbiEntry::Function(_) | AbiEntry::L1Handler(_) | AbiEntry::Constructor(_) => {
                vec![entry.clone()]
            }
            AbiEntry::Interface(interface) => flatten_abi_funcs_recursive(&interface.items),
            _ => vec![],
        })
        .collect()
}

pub fn get_entrypoint_name_from_class(class: &ClassAbi, selector: Felt) -> Option<String> {
    match class {
        ClassAbi::Sierra((entrypoints, abi)) => {
            let entrypoint_idx = match entrypoints
                .external
                .iter()
                .chain(entrypoints.l1_handler.iter())
                .chain(entrypoints.constructor.iter())
                .find(|entrypoint| entrypoint.selector == selector)
            {
                Some(entrypoint) => entrypoint.function_idx,
                None => return None,
            };

            abi.get(entrypoint_idx as usize).and_then(|function| match function {
                AbiEntry::Function(function) => Some(function.name.clone()),
                AbiEntry::L1Handler(l1_handler) => Some(l1_handler.name.clone()),
                AbiEntry::Constructor(constructor) => Some(constructor.name.clone()),
                _ => None,
            })
        }
        ClassAbi::Legacy(abi) => abi.iter().find_map(|entry| match entry {
            LegacyContractAbiEntry::Function(function)
                if get_selector_from_name(&function.name).unwrap() == selector =>
            {
                Some(function.name.clone())
            }
            _ => None,
        }),
    }
}
