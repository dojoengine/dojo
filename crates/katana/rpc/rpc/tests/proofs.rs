use std::path::PathBuf;

use dojo_test_utils::sequencer::{get_default_test_config, TestSequencer};
use jsonrpsee::http_client::HttpClientBuilder;
use katana_node::config::SequencingConfig;
use katana_primitives::block::BlockIdOrTag;
use katana_primitives::hash;
use katana_primitives::hash::StarkHash;
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
