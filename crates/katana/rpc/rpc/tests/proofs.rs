use std::path::PathBuf;

use assert_matches::assert_matches;
use dojo_test_utils::sequencer::{get_default_test_config, TestSequencer};
use jsonrpsee::http_client::HttpClientBuilder;
use katana_node::config::rpc::DEFAULT_RPC_MAX_PROOF_KEYS;
use katana_node::config::SequencingConfig;
use katana_primitives::block::BlockIdOrTag;
use katana_primitives::class::ClassHash;
use katana_primitives::hash::StarkHash;
use katana_primitives::{hash, Felt};
use katana_rpc_api::starknet::StarknetApiClient;
use katana_rpc_types::trie::GetStorageProofResponse;
use katana_trie::bitvec::view::AsBits;
use katana_trie::bonsai::BitVec;
use katana_trie::MultiProof;
use starknet::accounts::Account;
use starknet::core::types::BlockTag;
use starknet::macros::short_string;

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
async fn classes_proofs() {
    let sequencer =
        TestSequencer::start(get_default_test_config(SequencingConfig::default())).await;

    let provider = sequencer.provider();
    let account = sequencer.account();

    let path: PathBuf = PathBuf::from("tests/test_data/cairo1_contract.json");
    let (contract, compiled_class_hash) = common::prepare_contract_declaration_params(&path)
        .expect("failed to prepare class declaration params");

    let class_hash = contract.class_hash();
    let res = account
        .declare_v2(contract.into(), compiled_class_hash)
        .send()
        .await
        .expect("failed to send declare tx");

    dojo_utils::TransactionWaiter::new(res.transaction_hash, &provider)
        .await
        .expect("failed to wait on tx");

    // We need to use the jsonrpsee client because `starknet-rs` doesn't yet support RPC 0.8.0
    let client = HttpClientBuilder::default().build(sequencer.url()).unwrap();

    let GetStorageProofResponse { global_roots, classes_proof, .. } = client
        .get_storage_proof(BlockIdOrTag::Tag(BlockTag::Latest), Some(vec![class_hash]), None, None)
        .await
        .expect("failed to get storage proof");

    let key: BitVec = class_hash.to_bytes_be().as_bits()[5..].to_owned();
    let value =
        hash::Poseidon::hash(&short_string!("CONTRACT_CLASS_LEAF_V0"), &compiled_class_hash);

    let classes_proof = MultiProof::from(classes_proof.nodes);

    // the returned data is the list of values corresponds to the [key]
    let results = classes_proof
        .verify_proof::<hash::Poseidon>(global_roots.classes_tree_root, [key], 251)
        .collect::<Result<Vec<_>, _>>()
        .expect("failed to verify proofs");

    assert_eq!(vec![value], results);
}
