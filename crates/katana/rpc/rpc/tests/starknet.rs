use std::fs::{self};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use dojo_test_utils::anvil::TestAnvil;
use dojo_test_utils::sequencer::{get_default_test_starknet_config, TestSequencer};
use ethers::{contract::ContractFactory, types::H160};
use ethers_solc::{Artifact, Project, ProjectPathsConfig};
use katana_core::sequencer::SequencerConfig;
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
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;
    let account = sequencer.account();

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

    sequencer.stop().expect("failed to stop sequencer");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_send_declare_and_deploy_legacy_contract() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;
    let account = sequencer.account();

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

    sequencer.stop().expect("failed to stop sequencer");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_messaging_L1_L2() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("test_data").join("solidity");
    let paths = ProjectPathsConfig::builder().root(&root).sources(&root).build().unwrap();

    // get the solc project instance using the paths above
    let project = Project::builder().paths(paths).ephemeral().no_artifacts().build().unwrap();

    // compile the project and get the artifacts
    let output = project.compile().context("Error compiling project").unwrap();
    output.assert_success();

    //Get both contracts
    let contract_starknet_messaging_local =
        output.find_first("StarknetMessagingLocal").expect("could not find contract").clone();

    let (abi_cstrk, bytecode_cstrk, _) = contract_starknet_messaging_local.into_parts();

    let contract_1 = output.find_first("Contract1").expect("could not find contract").clone();

    let (abi_c1, bytecode_c1, _) = contract_1.into_parts();

    let anvil: TestAnvil = TestAnvil::start().await;

    let account = Arc::new(anvil.account());

    // Factories to deploy the contracts
    let factory_cstrk =
        ContractFactory::new(abi_cstrk.clone().unwrap(), bytecode_cstrk.unwrap(), account.clone());

    let factory_c1 =
        ContractFactory::new(abi_c1.clone().unwrap(), bytecode_c1.unwrap(), account.clone());

    //Deploy to local node (anvil)
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

    // -----------------------------------------------------------------------------------------------------------------------------

    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;
    let account = sequencer.account();

    let path: PathBuf = PathBuf::from("tests/test_data/cairo_l1_msg_contract.json");
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

    //TODO Messaging between Layers

    sequencer.stop().expect("failed to stop sequencer");
    anvil.stop();
}
