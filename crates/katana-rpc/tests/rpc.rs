use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Ok, Result};
use starknet::core::types::contract::SierraClass;
use starknet::core::types::{
    BroadcastedDeclareTransaction, BroadcastedDeclareTransactionV2, FieldElement,
    FlattenedSierraClass,
};
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};
use starknet::providers::Provider;
use url::Url;

fn get_flattened_sierra_class(raw_contract_class: &str) -> Result<FlattenedSierraClass> {
    let contract_artifact: SierraClass = serde_json::from_str(raw_contract_class)?;
    Ok(contract_artifact.flatten()?)
}

#[ignore]
#[tokio::test]
async fn test_send_declare_v2_tx() {
    let provider =
        JsonRpcClient::new(HttpTransport::new(Url::parse("http://localhost:5050").unwrap()));

    let path: PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "tests/test_data/cairo1_contract.json"].iter().collect();

    let raw_contract_str = fs::read_to_string(path).unwrap();
    let contract_class = Arc::new(get_flattened_sierra_class(&raw_contract_str).unwrap());

    let res = provider
        .add_declare_transaction(&BroadcastedDeclareTransaction::V2(
            BroadcastedDeclareTransactionV2 {
                max_fee: FieldElement::ZERO,
                nonce: FieldElement::ZERO,
                sender_address: FieldElement::from_str(
                    "0x03819aca4f147e3b589807dd81257c02c4d616328f8b6bdc097b4ae517130a97",
                )
                .unwrap(),
                signature: vec![],
                compiled_class_hash: FieldElement::from_hex_be(
                    "0x3e8c2b461e33e7711995014afdd012b94e533cdee94ef951cf27f2489b62055",
                )
                .unwrap(),
                contract_class,
            },
        ))
        .await;

    println!("{res:?}");
    assert!(res.is_ok())
}
