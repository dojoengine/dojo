use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Ok, Result};
use starknet::core::chain_id;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::{
    BlockId, BlockTag, BroadcastedDeclareTransaction, BroadcastedDeclareTransactionV2,
    BroadcastedDeclareTransactionV1,
    FieldElement, FlattenedSierraClass,
    {contract::legacy::LegacyContractClass}
};
use starknet::core::utils::{get_contract_address, get_selector_from_name};
use starknet::macros::felt;
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};
use starknet::providers::Provider;
use starknet::signers::{LocalWallet, SigningKey};
use starknet_api::hash::StarkFelt;
use starknet_api::stark_felt;
use starknet_api::core::{ChainId};
use starknet::{contract::ContractFactory};
use url::Url;

// const KKRT_CLASS_HASH = "0x48f610ed2be617844c0483633e47d09a1cb8c482989f83be589904f5f67b308";
const KKRT_CLASS_HASH2: FieldElement =
    felt!("0x48f610ed2be617844c0483633e47d09a1cb8c482989f83be589904f5f67b308");

const JHNN_CLASS_HASH: FieldElement = felt!("0x25b44527e082db50ccaeb7b8e9973e2a9e8f073f6bb855c4fd8c100bbbbd7e3");

fn get_flattened_sierra_class(raw_contract_class: &str) -> Result<FlattenedSierraClass> {
    let contract_artifact: SierraClass = serde_json::from_str(raw_contract_class)?;
    Ok(contract_artifact.flatten()?)
}

use starknet::accounts::{Account, Call, ConnectedAccount, SingleOwnerAccount};

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
                max_fee: FieldElement::from_str("1").unwrap(),
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


#[tokio::test]
async fn test_send_declare_v1_tx() {
    let provider =
        JsonRpcClient::new(HttpTransport::new(Url::parse("http://localhost:5050").unwrap()));

    let path: PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "tests/test_data/contracts/compiled/test_contract.json"].iter().collect();
    
    let legacy_contract: LegacyContractClass =
        serde_json::from_reader(fs::File::open(path).unwrap()).unwrap();
    let contract_class = Arc::new(legacy_contract.compress().unwrap());
    let res = provider
        .add_declare_transaction(&BroadcastedDeclareTransaction::V1(
            BroadcastedDeclareTransactionV1 {
                max_fee: FieldElement::from_str("0").unwrap(),
                nonce: FieldElement::from_str("1").unwrap(),
                sender_address: FieldElement::from_str(
                    "0x078227d7703321a763d6530373404f17a3f6f430544c0874d26006a404b929ed",
                )
                .unwrap(),
                signature: vec![],
                contract_class,
            },
        ))
        .await;

    println!("{res:?}");
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_contract_deploy_via_udc() {
    // NOTE: you will need to declare this class first
    let chainId = FieldElement::from_hex_be("0x4b4154414e41").unwrap();

    // let path: PathBuf =
    //     [env!("CARGO_MANIFEST_DIR"), "tests/test_data/cairo0_contract.json"].iter().collect();

    
    let path: PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "tests/test_data/contracts/compiled/test_contract.json"].iter().collect();
    
    let contract_artifact: LegacyContractClass =
        serde_json::from_reader(std::fs::File::open(path).unwrap())
            .unwrap();
    let class_hash = contract_artifact.class_hash().unwrap();

    let provider =
        JsonRpcClient::new(HttpTransport::new(Url::parse("http://localhost:5050").unwrap()));

    let signer = LocalWallet::from(SigningKey::from_secret_scalar(
        FieldElement::from_hex_be(
            "0x00ea7dc42ff7fbed5a64adecfaa7fd340075f76c931190571d0f52cc1d4b9002",
        )
        .unwrap(),
    ));
    let address = FieldElement::from_hex_be(
        "0x078227d7703321a763d6530373404f17a3f6f430544c0874d26006a404b929ed",
    )
        .unwrap();

    let mut account = SingleOwnerAccount::new(provider, signer, address, chainId);
    
    
    // `SingleOwnerAccount` defaults to checking nonce and estimating fees against the latest
    // block. Optionally change the target block to pending with the following line:
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    // Wrapping in `Arc` is meaningless here. It's just showcasing it could be done as
    // `Arc<Account>` implements `Account` too.
    let account = Arc::new(account);

    println!("class h4sh: {:#02X}", class_hash);
    
    let contract_factory = ContractFactory::new(class_hash, account);
    contract_factory
        .deploy(&vec![felt!("123456"), felt!("123457")], felt!("1"), false)
        .send()
        .await
        .expect("Unable to deploy contract");
}
