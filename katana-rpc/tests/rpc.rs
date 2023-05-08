use std::{fs, str::FromStr};

use katana_rpc::util::{get_casm_class_hash, get_flattened_sierra_class};
use starknet::{
    core::types::{contract::legacy::LegacyContractClass, FieldElement},
    providers::jsonrpc::{
        models::{
            BroadcastedDeclareTransaction, BroadcastedDeclareTransactionV1,
            BroadcastedDeclareTransactionV2, LegacyContractClass as BroadcastedLegacyContractClass,
            SierraContractClass,
        },
        HttpTransport, JsonRpcClient,
    },
};
use url::Url;

#[tokio::test]
async fn test_send_declare_v1_tx() {
    let provider = JsonRpcClient::new(HttpTransport::new(
        Url::parse("http://localhost:5050").unwrap(),
    ));

    let raw_contract_str = fs::read_to_string("/home/kari/project/work/dojoengine/katana/crates/katana-core/contracts/compiled/executor.json").unwrap();
    // let compiled_class_hash = get_casm_class_hash(&raw_contract_str).unwrap();
    // println!("{:#x}");
    let contract =
        serde_json::to_string(&get_flattened_sierra_class(&raw_contract_str).unwrap()).unwrap();
    let contract_class: SierraContractClass = serde_json::from_str(&contract).unwrap();

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
    // assert!(res.is_ok())
}
