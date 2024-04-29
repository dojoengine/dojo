use std::fs::{self};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use dojo_world::utils::TransactionWaiter;
use ethers::contract::ContractFactory;
use ethers::types::{H160, U256};
use ethers_contract::abigen;
use ethers_solc::{Artifact, Project, ProjectPathsConfig};
use katana_runner::{AnvilRunner, KatanaRunner};
use starknet::accounts::{Account, Call, ConnectedAccount};
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::{
    BlockId, BlockTag, DeclareTransactionReceipt, FieldElement, MaybePendingTransactionReceipt,
    TransactionFinalityStatus, TransactionReceipt,
};
use starknet::core::utils::{get_contract_address, get_selector_from_name};
use starknet::providers::Provider;
mod common;

const WAIT_TX_DELAY_MILLIS: u64 = 1000;

#[tokio::test(flavor = "multi_thread")]
async fn test_send_declare_and_deploy_contract() {
    let katana_runner = KatanaRunner::new().unwrap();
    let account = katana_runner.account(0);

    let path: PathBuf = PathBuf::from("tests/test_data/cairo1_contract.json");
    let (contract, compiled_class_hash) =
        common::prepare_contract_declaration_params(&path).unwrap();

    let class_hash = contract.class_hash();
    let res = account.declare(Arc::new(contract), compiled_class_hash).send().await.unwrap();

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(WAIT_TX_DELAY_MILLIS)).await;

    let receipt = account.provider().get_transaction_receipt(res.transaction_hash).await.unwrap();

    match receipt {
        MaybePendingTransactionReceipt::Receipt(TransactionReceipt::Declare(
            DeclareTransactionReceipt { finality_status, .. },
        )) => {
            assert_eq!(finality_status, TransactionFinalityStatus::AcceptedOnL2);
        }
        _ => panic!("invalid tx receipt"),
    }

    assert!(account.provider().get_class(BlockId::Tag(BlockTag::Latest), class_hash).await.is_ok());

    let constructor_calldata = vec![FieldElement::from(1_u32), FieldElement::from(2_u32)];

    let calldata = [
        vec![
            res.class_hash,                                 // class hash
            FieldElement::ZERO,                             // salt
            FieldElement::ZERO,                             // unique
            FieldElement::from(constructor_calldata.len()), // constructor calldata len
        ],
        constructor_calldata.clone(),
    ]
    .concat();

    let contract_address = get_contract_address(
        FieldElement::ZERO,
        res.class_hash,
        &constructor_calldata,
        FieldElement::ZERO,
    );

    account
        .execute(vec![Call {
            calldata,
            // devnet UDC address
            to: FieldElement::from_hex_be(
                "0x41a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf",
            )
            .unwrap(),
            selector: get_selector_from_name("deployContract").unwrap(),
        }])
        .send()
        .await
        .unwrap();

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(WAIT_TX_DELAY_MILLIS)).await;

    assert_eq!(
        account
            .provider()
            .get_class_hash_at(BlockId::Tag(BlockTag::Latest), contract_address)
            .await
            .unwrap(),
        class_hash
    );

    drop(katana_runner);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_send_declare_and_deploy_legacy_contract() {
    let katana_runner = KatanaRunner::new().unwrap();
    let account = katana_runner.account(0);

    let path = PathBuf::from("tests/test_data/cairo0_contract.json");

    let legacy_contract: LegacyContractClass =
        serde_json::from_reader(fs::File::open(path).unwrap()).unwrap();
    let contract_class = Arc::new(legacy_contract);

    let class_hash = contract_class.class_hash().unwrap();
    let res = account.declare_legacy(contract_class).send().await.unwrap();
    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(WAIT_TX_DELAY_MILLIS)).await;

    let receipt = account.provider().get_transaction_receipt(res.transaction_hash).await.unwrap();

    match receipt {
        MaybePendingTransactionReceipt::Receipt(TransactionReceipt::Declare(
            DeclareTransactionReceipt { finality_status, .. },
        )) => {
            assert_eq!(finality_status, TransactionFinalityStatus::AcceptedOnL2);
        }
        _ => panic!("invalid tx receipt"),
    }

    assert!(account.provider().get_class(BlockId::Tag(BlockTag::Latest), class_hash).await.is_ok());

    let constructor_calldata = vec![FieldElement::ONE];

    let calldata = [
        vec![
            res.class_hash,                                 // class hash
            FieldElement::ZERO,                             // salt
            FieldElement::ZERO,                             // unique
            FieldElement::from(constructor_calldata.len()), // constructor calldata len
        ],
        constructor_calldata.clone(),
    ]
    .concat();

    let contract_address = get_contract_address(
        FieldElement::ZERO,
        res.class_hash,
        &constructor_calldata.clone(),
        FieldElement::ZERO,
    );

    account
        .execute(vec![Call {
            calldata,
            // devnet UDC address
            to: FieldElement::from_hex_be(
                "0x41a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf",
            )
            .unwrap(),
            selector: get_selector_from_name("deployContract").unwrap(),
        }])
        .send()
        .await
        .unwrap();

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(WAIT_TX_DELAY_MILLIS)).await;

    assert_eq!(
        account
            .provider()
            .get_class_hash_at(BlockId::Tag(BlockTag::Latest), contract_address)
            .await
            .unwrap(),
        class_hash
    );

    drop(katana_runner)
}

abigen!(
        Contract1_test,
        "/Users/fabrobles/Fab/dojo_fork/crates/katana/rpc/rpc/tests/test_data/solidity/Contract1_abi2.json",
        event_derives(serde::Serialize, serde::Deserialize)
    );

#[tokio::test(flavor = "multi_thread")]
async fn test_messaging_l1_l2() {
    // Prepare Anvil + Messaging Contracts
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("test_data").join("solidity");
    let paths = ProjectPathsConfig::builder().root(&root).sources(&root).build().unwrap();

    let project = Project::builder().paths(paths).ephemeral().no_artifacts().build().unwrap();

    let output = project.compile().context("Error compiling project").unwrap();
    output.assert_success();

    let contract_starknet_messaging_local =
        output.find_first("StarknetMessagingLocal").expect("could not find contract").clone();
    let (abi_cstrk, bytecode_cstrk, _) = contract_starknet_messaging_local.into_parts();

    let contract_1 = output.find_first("Contract1").expect("could not find contract").clone();
    let (abi_c1, bytecode_c1, _) = contract_1.into_parts();

    let anvil_runner = AnvilRunner::new().await.unwrap();

    let eth_account = Arc::new(anvil_runner.account().await);

    let factory_cstrk = ContractFactory::new(
        abi_cstrk.clone().unwrap(),
        bytecode_cstrk.unwrap(),
        eth_account.clone(),
    );
    let factory_c1 =
        ContractFactory::new(abi_c1.clone().unwrap(), bytecode_c1.unwrap(), eth_account.clone());

    // Deploy to local node (anvil)
    let contract_strk =
        factory_cstrk.deploy(()).expect("Failing deploying").send().await.expect("Failing sending");

    assert_eq!(
        contract_strk.address(),
        H160::from_str("0x5fbdb2315678afecb367f032d93f642f64180aa3").unwrap()
    );

    let contract_c1 = factory_c1
        .deploy(contract_strk.address())
        .expect("Failing deploying")
        .send()
        .await
        .expect("Failing sending");

    assert_eq!(
        contract_c1.address(),
        H160::from_str("0xe7f1725e7734ce288f8367e1bb143e90bb3f0512").unwrap()
    );

    // Prepare Katana + Messaging Contract
    let katana_runner = KatanaRunner::new().unwrap();
    let starknet_account = katana_runner.account(0);

    let path: PathBuf = PathBuf::from("tests/test_data/cairo_l1_msg_contract.json");
    let (contract, compiled_class_hash) =
        common::prepare_contract_declaration_params(&path).unwrap();

    let class_hash = contract.class_hash();
    let res =
        starknet_account.declare(Arc::new(contract), compiled_class_hash).send().await.unwrap();

    let receipt = TransactionWaiter::new(res.transaction_hash, starknet_account.provider())
        .with_tx_status(TransactionFinalityStatus::AcceptedOnL2)
        .await
        .expect("Invalid tx receipt");

    assert_eq!(receipt.finality_status(), &TransactionFinalityStatus::AcceptedOnL2);

    assert!(starknet_account
        .provider()
        .get_class(BlockId::Tag(BlockTag::Latest), class_hash)
        .await
        .is_ok());

    let constructor_calldata = vec![FieldElement::from(1_u32), FieldElement::from(2_u32)];

    let contract_address = get_contract_address(
        FieldElement::ZERO,
        res.class_hash,
        &constructor_calldata,
        FieldElement::ZERO,
    );

    assert_eq!(
        contract_address,
        FieldElement::from_str(
            "0x0024f0deadb642bd4792c19da937758eb6bf4747d7d93a13ecc527bba82eb1f1"
        )
        .unwrap()
    );

    //Messaging between L1 -> L2

    let contr1_test = Contract1_test::new(contract_c1.address(), eth_account.clone());
    let address =
        U256::from_str("0x0024f0deadb642bd4792c19da937758eb6bf4747d7d93a13ecc527bba82eb1f1")
            .unwrap();
    let selector =
        U256::from_str("0x005421de947699472df434466845d68528f221a52fce7ad2934c5dae2e1f1cdc")
            .unwrap();
    let payload = vec![U256::from(123)];
    let receipt = contr1_test
        .send_message(address, selector, payload)
        .gas(10000000)
        .send()
        .await
        .unwrap()
        .await
        .unwrap();
    println!("RECEIPT {:?}", receipt);

    drop(katana_runner);
    drop(anvil_runner);
}
