//! Utilities for parsing the logs in JSON format. This is when katana is run with `--json-log`.

use serde::Deserialize;

#[derive(Deserialize)]
pub struct JsonLogMessage {
    pub timestamp: String,
    pub level: String,
    pub fields: JsonLogFields,
    pub target: String,
}

#[derive(Deserialize)]
pub struct JsonLogFields {
    #[serde(deserialize_with = "deserialize_katana_info")]
    pub message: KatanaInfo,
}

fn deserialize_katana_info<'de, D>(deserializer: D) -> Result<KatanaInfo, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    serde_json::from_str(&s).map_err(serde::de::Error::custom)
}

#[derive(Deserialize)]
pub struct KatanaInfo {
    pub seed: String,
    pub address: String,
    pub accounts: Vec<(String, AccountInfo)>,
}

#[derive(Deserialize)]
pub struct AccountInfo {
    pub balance: String,
    pub class_hash: String,
    pub private_key: String,
    pub public_key: String,
}
