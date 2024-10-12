#![allow(dead_code)]

//! Utilities for parsing the logs in JSON format. This is when katana is run with `--json-log`.
//!
//! When JSON log is enabled, the startup details are all printed in a single log message.

use std::net::SocketAddr;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct JsonLog<T = serde_json::Value> {
    pub timestamp: String,
    pub level: String,
    pub fields: Fields<T>,
    pub target: String,
}

#[derive(Deserialize, Debug)]
pub struct Fields<T = serde_json::Value> {
    pub message: String,
    #[serde(flatten)]
    pub other: T,
}

/// Katana startup log message. The object is included as a string in the `message` field. Hence we
/// have to parse it separately unlike the [`RpcAddr`] where we can directly deserialize using the
/// Fields generic parameter.
///
/// Example:
///
/// ```json
/// {
///   "timestamp": "2024-10-10T14:55:04.452924Z",
///   "level": "INFO",
///   "fields": {
///     "message": "{\"accounts\":[[\"0x2af9427c5a277474c079a1283c880ee8a6f0f8fbf73ce969c08d88befec1bba\",{\"balance\":\"0x21e19e0c9bab2400000\",\"class_hash\":\"0x5400e90f7e0ae78bd02c77cd75527280470e2fe19c54970dd79dc37a9d3645c\",\"private_key\":\"0x1800000000300000180000000000030000000000003006001800006600\",\"public_key\":\"0x2b191c2f3ecf685a91af7cf72a43e7b90e2e41220175de5c4f7498981b10053\"}]],\"seed\":\"0\"}"
///   },
///   "target": "katana::cli"
/// }
/// ```
#[derive(Deserialize, Debug)]
pub struct KatanaInfo {
    pub seed: String,
    pub accounts: Vec<(String, AccountInfo)>,
}

impl TryFrom<String> for KatanaInfo {
    type Error = serde_json::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        serde_json::from_str(&value)
    }
}

#[derive(Deserialize, Debug)]
pub struct AccountInfo {
    pub balance: String,
    pub class_hash: String,
    pub private_key: String,
    pub public_key: String,
}

/// {
///     "message": "RPC server started.",
///     "addr": "127.0.0.1:5050"
/// }
#[derive(Deserialize, Debug)]
pub struct RpcAddr {
    pub addr: SocketAddr,
}
