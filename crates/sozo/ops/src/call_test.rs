use anyhow::Result;
use dojo_test_utils::compiler::build_test_config;
use dojo_test_utils::migration::prepare_migration;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, TestSequencer,
};
use dojo_world::contracts::world::WorldContract;
use scarb::ops;
use starknet::accounts::SingleOwnerAccount;
use starknet::core::types::{BlockId, BlockTag};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::LocalWallet;
use starknet_crypto::FieldElement;

use crate::{call, get_contract_address, migration};

const CONTRACT_NAME: &str = "dojo_examples::actions::actions";
const ENTRYPOINT: &str = "tile_terrain";

async fn setup(
    sequencer: &TestSequencer,
) -> Result<WorldContract<SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>>> {
    let config = build_test_config("../../../examples/spawn-and-move/Scarb.toml")?;
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));
    let base_dir = "../../../examples/spawn-and-move";
    let target_dir = format!("{}/target/dev", base_dir);

    let mut migration = prepare_migration(base_dir.into(), target_dir.into())?;

    // no need for models
    migration.models = vec![];

    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let output = migration::execute_strategy(&ws, &migration, &account, None).await?;
    let world = WorldContract::new(output.world_address, account);

    Ok(world)
}

#[test]
fn parse_block_id_bad_hash() {
    assert!(call::parse_block_id(Some("0xBadHash".to_string())).is_err());
}

#[test]
fn parse_block_id_bad_string() {
    assert!(call::parse_block_id(Some("BadString".to_string())).is_err());
}

#[test]
fn parse_block_id_none() {
    assert!(call::parse_block_id(None).unwrap() == BlockId::Tag(BlockTag::Pending));
}

#[test]
fn parse_block_id_hash() {
    assert!(
        call::parse_block_id(Some("0x1234".to_string())).unwrap()
            == BlockId::Hash(FieldElement::from_hex_be("0x1234").unwrap())
    );
}

#[test]
fn parse_block_id_pending() {
    assert!(
        call::parse_block_id(Some("pending".to_string())).unwrap()
            == BlockId::Tag(BlockTag::Pending)
    );
}

#[test]
fn parse_block_id_latest() {
    assert!(
        call::parse_block_id(Some("latest".to_string())).unwrap() == BlockId::Tag(BlockTag::Latest)
    );
}

#[test]
fn parse_block_id_number() {
    assert!(call::parse_block_id(Some("42".to_string())).unwrap() == BlockId::Number(42));
}

#[tokio::test]
async fn call_with_bad_address() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    if let Ok(world) = setup(&sequencer).await {
        assert!(
            call::call(
                "0xBadCoffeeBadCode".to_string(),
                ENTRYPOINT.to_string(),
                vec![FieldElement::ZERO, FieldElement::ZERO],
                world,
                None
            )
            .await
            .is_err()
        );
    } else {
        panic!("Unable to setup the test");
    }
}

#[tokio::test]
async fn call_with_bad_name() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    if let Ok(world) = setup(&sequencer).await {
        assert!(
            call::call(
                "BadName".to_string(),
                ENTRYPOINT.to_string(),
                vec![FieldElement::ZERO, FieldElement::ZERO],
                world,
                None
            )
            .await
            .is_err()
        );
    } else {
        panic!("Unable to setup the test");
    }
}

#[tokio::test]
async fn call_with_bad_entrypoint() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    if let Ok(world) = setup(&sequencer).await {
        assert!(
            call::call(
                CONTRACT_NAME.to_string(),
                "BadEntryPoint".to_string(),
                vec![FieldElement::ZERO, FieldElement::ZERO],
                world,
                None
            )
            .await
            .is_err()
        );
    } else {
        panic!("Unable to setup the test");
    }
}

#[tokio::test]
async fn call_with_bad_calldata() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    if let Ok(world) = setup(&sequencer).await {
        assert!(
            call::call(CONTRACT_NAME.to_string(), ENTRYPOINT.to_string(), vec![], world, None)
                .await
                .is_err()
        );
    } else {
        panic!("Unable to setup the test");
    }
}

#[tokio::test]
async fn call_with_contract_name() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    if let Ok(world) = setup(&sequencer).await {
        assert!(
            call::call(
                CONTRACT_NAME.to_string(),
                ENTRYPOINT.to_string(),
                vec![FieldElement::ZERO, FieldElement::ZERO],
                world,
                None,
            )
            .await
            .is_ok()
        );
    } else {
        panic!("Unable to setup the test");
    }
}

#[tokio::test]
async fn call_with_contract_address() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    if let Ok(world) = setup(&sequencer).await {
        let contract_address = get_contract_address::<
            SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
        >(&world, CONTRACT_NAME.to_string())
        .await
        .unwrap();

        assert!(
            call::call(
                format!("{:#x}", contract_address),
                ENTRYPOINT.to_string(),
                vec![FieldElement::ZERO, FieldElement::ZERO],
                world,
                None,
            )
            .await
            .is_ok()
        );
    } else {
        panic!("Unable to setup the test");
    }
}
