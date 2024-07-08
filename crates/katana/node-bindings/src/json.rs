//! Utilities for parsing the logs in JSON format. This is when katana is run with `--json-log`.
//!
//! When JSON log is enabled, the startup details are all printed in a single log message.
//! Example startup log in JSON format:
//!
//! ```json
//! {"timestamp":"2024-07-06T03:35:00.410846Z","level":"INFO","fields":{"message":"{\"accounts\":[[\
//! "318027405971194400117186968443431282813445578359155272415686954645506762954\",{\"balance\":\"
//! 0x21e19e0c9bab2400000\",\"class_hash\":\"
//! 0x5400e90f7e0ae78bd02c77cd75527280470e2fe19c54970dd79dc37a9d3645c\",\"private_key\":\"
//! 0x2bbf4f9fd0bbb2e60b0316c1fe0b76cf7a4d0198bd493ced9b8df2a3a24d68a\",\"public_key\":\"
//! 0x640466ebd2ce505209d3e5c4494b4276ed8f1cde764d757eb48831961f7cdea\"}]],\"address\":\"0.0.0.0:
//! 5050\",\"seed\":\"0\"}"},"target":"katana::cli"}
//! ```
#![allow(dead_code)]

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
