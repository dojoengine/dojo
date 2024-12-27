use std::path::PathBuf;

use assert_matches::assert_matches;
use dojo_test_utils::sequencer::{get_default_test_config, TestSequencer};
use jsonrpsee::http_client::HttpClientBuilder;
use katana_node::config::rpc::DEFAULT_RPC_MAX_PROOF_KEYS;
use katana_node::config::SequencingConfig;
use katana_primitives::block::BlockIdOrTag;
use katana_primitives::class::{ClassHash, CompiledClassHash};
use katana_primitives::contract::{StorageKey, StorageValue};
use katana_primitives::{hash, ContractAddress, Felt};
use katana_rpc_api::starknet::StarknetApiClient;
use katana_rpc_types::trie::ContractStorageKeys;
use katana_trie::{
    compute_classes_trie_value, compute_contract_state_hash, ClassesMultiProof, MultiProof,
};
use starknet::accounts::{Account, ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::BlockTag;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::LocalWallet;

mod common;

#[tokio::test]
async fn proofs_limit() {
    use jsonrpsee::core::Error;
    use jsonrpsee::types::error::CallError;
    use serde_json::json;

    let sequencer =
        TestSequencer::start(get_default_test_config(SequencingConfig::default())).await;

    // We need to use the jsonrpsee client because `starknet-rs` doesn't yet support RPC 0
    let client = HttpClientBuilder::default().build(sequencer.url()).unwrap();

    // Because we're using the default configuration for instantiating the node, the RPC limit is
    // set to 100. The total keys is 35 + 35 + 35 = 105.

    // Generate dummy keys
    let mut classes = Vec::new();
    let mut contracts = Vec::new();
    let mut storages = Vec::new();

    for i in 0..35 {
        storages.push(Default::default());
        classes.push(ClassHash::from(i as u64));
        contracts.push(Felt::from(i as u64).into());
    }

    let err = client
        .get_storage_proof(
            BlockIdOrTag::Tag(BlockTag::Latest),
            Some(classes),
            Some(contracts),
            Some(storages),
        )
        .await
        .expect_err("rpc should enforce limit");

    assert_matches!(err, Error::Call(CallError::Custom(e)) => {
        assert_eq!(e.code(), 1000);
        assert_eq!(&e.message(), &"Proof limit exceeded");

        let expected_data = json!({
            "total": 105,
            "limit": DEFAULT_RPC_MAX_PROOF_KEYS,
        });

        let actual_data = e.data().expect("must have data");
        let actual_data = serde_json::to_value(actual_data).unwrap();

        assert_eq!(actual_data, expected_data);
    });
}

#[tokio::test]
async fn genesis_states() {
    let cfg = get_default_test_config(SequencingConfig::default());

    let sequencer = TestSequencer::start(cfg).await;
    let genesis_states = sequencer.backend().chain_spec.state_updates();

    // We need to use the jsonrpsee client because `starknet-rs` doesn't yet support RPC 0.8.0
    let client = HttpClientBuilder::default().build(sequencer.url()).unwrap();

    // Check class declarations
    let genesis_classes =
        genesis_states.state_updates.declared_classes.keys().cloned().collect::<Vec<ClassHash>>();

    // Check contract deployments
    let genesis_contracts = genesis_states
        .state_updates
        .deployed_contracts
        .keys()
        .cloned()
        .collect::<Vec<ContractAddress>>();

    // Check contract storage
    let genesis_contract_storages = genesis_states
        .state_updates
        .storage_updates
        .iter()
        .map(|(address, keys)| ContractStorageKeys {
            address: *address,
            keys: keys.keys().cloned().collect(),
        })
        .collect::<Vec<ContractStorageKeys>>();

    let proofs = client
        .get_storage_proof(
            BlockIdOrTag::Tag(BlockTag::Latest),
            Some(genesis_classes.clone()),
            Some(genesis_contracts.clone()),
            Some(genesis_contract_storages.clone()),
        )
        .await
        .expect("failed to get state proofs");

    // -----------------------------------------------------------------------
    // Verify classes proofs

    let classes_proof = MultiProof::from(proofs.classes_proof.nodes);
    let classes_tree_root = proofs.global_roots.classes_tree_root;
    let classes_verification_result = katana_trie::verify_proof::<hash::Pedersen>(
        &classes_proof,
        classes_tree_root,
        genesis_classes,
    );

    // Compute the classes trie values
    let class_trie_entries = genesis_states
        .state_updates
        .declared_classes
        .values()
        .map(|compiled_hash| compute_classes_trie_value(*compiled_hash))
        .collect::<Vec<Felt>>();

    assert_eq!(class_trie_entries, classes_verification_result);

    // -----------------------------------------------------------------------
    // Verify contracts proofs

    let contracts_proof = MultiProof::from(proofs.contracts_proof.nodes);
    let contracts_tree_root = proofs.global_roots.contracts_tree_root;
    let contracts_verification_result = katana_trie::verify_proof::<hash::Pedersen>(
        &contracts_proof,
        contracts_tree_root,
        genesis_contracts.into_iter().map(Felt::from).collect(),
    );

    // Compute the classes trie values
    let contracts_trie_entries = proofs
        .contracts_proof
        .contract_leaves_data
        .into_iter()
        .map(|d| compute_contract_state_hash(&d.class_hash, &d.storage_root, &d.nonce))
        .collect::<Vec<Felt>>();

    assert_eq!(contracts_trie_entries, contracts_verification_result);

    // -----------------------------------------------------------------------
    // Verify contracts proofs

    let storages_updates = &genesis_states.state_updates.storage_updates.values();
    let storages_proofs = proofs.contracts_storage_proofs.nodes;

    // The order of which the proofs are returned is of the same order of the proofs requests.
    for (storages, proofs) in storages_updates.clone().zip(storages_proofs) {
        let storage_keys = storages.keys().cloned().collect::<Vec<StorageKey>>();
        let storage_values = storages.values().cloned().collect::<Vec<StorageValue>>();

        let contracts_storages_proof = MultiProof::from(proofs);
        let (storage_tree_root, ..) = contracts_storages_proof.0.first().unwrap();

        let storages_verification_result = katana_trie::verify_proof::<hash::Pedersen>(
            &contracts_storages_proof,
            *storage_tree_root,
            storage_keys,
        );

        assert_eq!(storage_values, storages_verification_result);
    }
}

#[tokio::test]
async fn classes_proofs() {
    let cfg = get_default_test_config(SequencingConfig::default());

    let sequencer = TestSequencer::start(cfg).await;
    let account = sequencer.account();

    let (class_hash1, compiled_class_hash1) =
        declare(&account, "tests/test_data/cairo1_contract.json").await;
    let (class_hash2, compiled_class_hash2) =
        declare(&account, "tests/test_data/cairo_l1_msg_contract.json").await;
    let (class_hash3, compiled_class_hash3) =
        declare(&account, "tests/test_data/test_sierra_contract.json").await;

    // We need to use the jsonrpsee client because `starknet-rs` doesn't yet support RPC 0.8.0
    let client = HttpClientBuilder::default().build(sequencer.url()).unwrap();

    {
        let class_hash = class_hash1;
        let trie_entry = compute_classes_trie_value(compiled_class_hash1);

        let proofs = client
            .get_storage_proof(BlockIdOrTag::Number(1), Some(vec![class_hash]), None, None)
            .await
            .expect("failed to get storage proof");

        let results = ClassesMultiProof::from(MultiProof::from(proofs.classes_proof.nodes))
            .verify(proofs.global_roots.classes_tree_root, vec![class_hash]);

        assert_eq!(vec![trie_entry], results);
    }

    {
        let class_hash = class_hash2;
        let trie_entry = compute_classes_trie_value(compiled_class_hash2);

        let proofs = client
            .get_storage_proof(BlockIdOrTag::Number(2), Some(vec![class_hash]), None, None)
            .await
            .expect("failed to get storage proof");

        let results = ClassesMultiProof::from(MultiProof::from(proofs.classes_proof.nodes))
            .verify(proofs.global_roots.classes_tree_root, vec![class_hash]);

        assert_eq!(vec![trie_entry], results);
    }

    {
        let class_hash = class_hash3;
        let trie_entry = compute_classes_trie_value(compiled_class_hash3);

        let proofs = client
            .get_storage_proof(BlockIdOrTag::Number(3), Some(vec![class_hash]), None, None)
            .await
            .expect("failed to get storage proof");

        let results = ClassesMultiProof::from(MultiProof::from(proofs.classes_proof.nodes))
            .verify(proofs.global_roots.classes_tree_root, vec![class_hash]);

        assert_eq!(vec![trie_entry], results);
    }

    {
        let class_hashes = vec![class_hash1, class_hash2, class_hash3];
        let trie_entries = vec![
            compute_classes_trie_value(compiled_class_hash1),
            compute_classes_trie_value(compiled_class_hash2),
            compute_classes_trie_value(compiled_class_hash3),
        ];

        let proofs = client
            .get_storage_proof(
                BlockIdOrTag::Tag(BlockTag::Latest),
                Some(class_hashes.clone()),
                None,
                None,
            )
            .await
            .expect("failed to get storage proof");

        let results = ClassesMultiProof::from(MultiProof::from(proofs.classes_proof.nodes))
            .verify(proofs.global_roots.classes_tree_root, class_hashes.clone());

        assert_eq!(trie_entries, results);
    }
}

async fn declare(
    account: &SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
    path: impl Into<PathBuf>,
) -> (ClassHash, CompiledClassHash) {
    let (contract, compiled_class_hash) = common::prepare_contract_declaration_params(&path.into())
        .expect("failed to prepare class declaration params");

    let class_hash = contract.class_hash();
    let res = account
        .declare_v2(contract.into(), compiled_class_hash)
        .send()
        .await
        .expect("failed to send declare tx");

    dojo_utils::TransactionWaiter::new(res.transaction_hash, account.provider())
        .await
        .expect("failed to wait on tx");

    (class_hash, compiled_class_hash)
}
