use core::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use dojo_types::schema::Ty;
use dojo_world::contracts::abigen::model::Layout;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteRow, FromRow, Row, Sqlite};
use starknet::core::types::Felt;

use crate::utils::map_column_decode_error;

#[derive(FromRow, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    pub keys: String,
    pub event_id: String,
    pub executed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // this should never be None
    #[sqlx(skip)]
    pub updated_model: Option<Ty>,
    #[sqlx(skip)]
    pub deleted: bool,
}

#[derive(FromRow, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OptimisticEntity {
    pub id: String,
    pub keys: String,
    pub event_id: String,
    pub executed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // this should never be None
    #[sqlx(skip)]
    pub updated_model: Option<Ty>,
    #[sqlx(skip)]
    pub deleted: bool,
}

#[derive(FromRow, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EventMessage {
    pub id: String,
    pub keys: String,
    pub event_id: String,
    pub executed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // this should never be None
    #[sqlx(skip)]
    pub updated_model: Option<Ty>,
    #[sqlx(skip)]
    pub historical: bool,
}

#[derive(FromRow, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OptimisticEventMessage {
    pub id: String,
    pub keys: String,
    pub event_id: String,
    pub executed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // this should never be None
    #[sqlx(skip)]
    pub updated_model: Option<Ty>,
    #[sqlx(skip)]
    pub historical: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub selector: Felt,
    pub namespace: String,
    pub name: String,
    pub class_hash: Felt,
    pub packed_size: u32,
    pub unpacked_size: u32,
    pub contract_address: Felt,
    pub layout: Layout,
    pub schema: Ty,
    pub executed_at: DateTime<Utc>,
}

impl FromRow<'_, SqliteRow> for Model {
    fn from_row(row: &'_ SqliteRow) -> sqlx::Result<Self> {
        let selector = row.try_get::<String, &str>("id")?;
        let class_hash = row.try_get::<String, &str>("class_hash")?;
        let contract_address = row.try_get::<String, &str>("contract_address")?;
        let layout = row.try_get::<String, &str>("layout")?;
        let schema = row.try_get::<String, &str>("schema")?;

        Ok(Model {
            selector: Felt::from_str(&selector)
                .map_err(|e| map_column_decode_error("id", Box::new(e)))?,
            namespace: row.get("namespace"),
            name: row.get("name"),
            class_hash: Felt::from_str(&class_hash)
                .map_err(|e| map_column_decode_error("class_hash", Box::new(e)))?,
            packed_size: row.get("packed_size"),
            unpacked_size: row.get("unpacked_size"),
            contract_address: Felt::from_str(&contract_address)
                .map_err(|e| map_column_decode_error("contract_address", Box::new(e)))?,
            layout: serde_json::from_str(&layout)
                .map_err(|e| map_column_decode_error("layout", Box::new(e)))?,
            schema: serde_json::from_str(&schema)
                .map_err(|e| map_column_decode_error("schema", Box::new(e)))?,
            executed_at: row.get("executed_at"),
        })
    }
}

#[derive(FromRow, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub keys: String,
    pub data: String,
    pub transaction_hash: String,
    pub executed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub id: String,
    pub contract_address: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub metadata: String,
}

#[derive(FromRow, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TokenBalance {
    pub id: String,
    pub balance: String,
    pub account_address: String,
    pub contract_address: String,
    pub token_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct Contract {
    pub address: Felt,
    pub r#type: ContractType,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContractType {
    WORLD,
    ERC20,
    ERC721,
}

impl std::fmt::Display for Contract {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{:#x}", self.r#type, self.address)
    }
}

impl FromStr for ContractType {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_lowercase().as_str() {
            "world" => Ok(ContractType::WORLD),
            "erc20" => Ok(ContractType::ERC20),
            "erc721" => Ok(ContractType::ERC721),
            _ => Err(anyhow::anyhow!("Invalid ERC type: {}", input)),
        }
    }
}

impl std::fmt::Display for ContractType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContractType::WORLD => write!(f, "WORLD"),
            ContractType::ERC20 => write!(f, "ERC20"),
            ContractType::ERC721 => write!(f, "ERC721"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContractCursor {
    pub head: i64,
    pub tps: i64,
    pub last_block_timestamp: i64,
    pub contract_address: Felt,
    pub last_pending_block_tx: Option<Felt>,
    pub last_pending_block_contract_tx: Option<Felt>,
}

impl FromRow<'_, SqliteRow> for ContractCursor {
    fn from_row(row: &'_ SqliteRow) -> sqlx::Result<Self> {
        let contract_address = row.try_get::<String, &str>("contract_address")?;
        let last_pending_block_tx = row.try_get::<Option<String>, &str>("last_pending_block_tx")?;
        let last_pending_block_contract_tx =
            row.try_get::<Option<String>, &str>("last_pending_block_contract_tx")?;

        Ok(ContractCursor {
            head: row.get("head"),
            tps: row.get("tps"),
            last_block_timestamp: row.get("last_block_timestamp"),
            contract_address: Felt::from_str(&contract_address)
                .map_err(|e| map_column_decode_error("contract_address", Box::new(e)))?,
            last_pending_block_tx: last_pending_block_tx
                .map(|c| Felt::from_str(&c))
                .transpose()
                .map_err(|e| map_column_decode_error("last_pending_block_tx", Box::new(e)))?,
            last_pending_block_contract_tx: last_pending_block_contract_tx
                .map(|c| Felt::from_str(&c))
                .transpose()
                .map_err(|e| {
                    map_column_decode_error("last_pending_block_contract_tx", Box::new(e))
                })?,
        })
    }
}
