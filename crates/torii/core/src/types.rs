use core::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use starknet::core::types::FieldElement;

#[derive(Serialize, Deserialize)]
pub struct SQLFieldElement(pub FieldElement);

impl From<SQLFieldElement> for FieldElement {
    fn from(field_element: SQLFieldElement) -> Self {
        field_element.0
    }
}

impl TryFrom<String> for SQLFieldElement {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(SQLFieldElement(FieldElement::from_hex_be(&value)?))
    }
}

impl fmt::LowerHex for SQLFieldElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(FromRow, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    pub keys: String,
    pub event_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: String,
    pub name: String,
    pub class_hash: String,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub keys: String,
    pub data: String,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
}
