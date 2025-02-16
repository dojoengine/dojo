use katana_feeder_gateway::client::SequencerGateway;
use katana_feeder_gateway::types::{Block, ContractClass, StateUpdate, StateUpdateWithBlock};
use katana_primitives::block::{BlockIdOrTag, BlockNumber};
use katana_primitives::class::{CasmContractClass, ClassHash};
use katana_primitives::felt;
use rstest::rstest;

mod fixtures;

use fixtures::{gateway, test_data};

#[rstest]
#[case::pre_v0_7_0(0, test_data("pre_0.7.0/block/mainnet_genesis.json"))]
#[case::v0_7_0(2240, test_data("0.7.0/block/mainnet_2240.json"))]
#[case::v0_8_0(2500, test_data("0.8.0/block/mainnet_2500.json"))]
#[case::v0_9_0(2800, test_data("0.9.0/block/mainnet_2800.json"))]
#[case::v0_10_0(6500, test_data("0.10.0/block/mainnet_6500.json"))]
#[case::v0_11_1(65000, test_data("0.11.1/block/mainnet_65000.json"))]
#[case::v0_13_0(550000, test_data("0.13.0/block/mainnet_550000.json"))]
#[tokio::test]
async fn get_block(
    gateway: SequencerGateway,
    #[case] block_number: BlockNumber,
    #[case] expected: Block,
) {
    let id = BlockIdOrTag::Number(block_number);
    let block = gateway.get_block(id).await.unwrap();
    similar_asserts::assert_eq!(block, expected);
}

#[rstest]
#[case::v0_12_2(350000, test_data("0.12.2/state_update/mainnet_350000.json"))]
#[tokio::test]
async fn get_state_update(
    gateway: SequencerGateway,
    #[case] block_number: BlockNumber,
    #[case] expected: StateUpdate,
) {
    let id = BlockIdOrTag::Number(block_number);
    let state_update = gateway.get_state_update(id).await.unwrap();
    similar_asserts::assert_eq!(state_update, expected);
}

#[rstest]
#[case::v0_11_1(65000, test_data("0.11.1/block/mainnet_65000.json"))]
#[tokio::test]
async fn get_state_update_with_block(
    gateway: SequencerGateway,
    #[case] block_number: BlockNumber,
    #[case] expected: StateUpdateWithBlock,
) {
    let id = BlockIdOrTag::Number(block_number);
    let state_update = gateway.get_state_update_with_block(id).await.unwrap();
    similar_asserts::assert_eq!(state_update, expected);
}

#[rstest]
#[case::pre_v0_7_0(felt!("0x1"), 0, test_data("0.7.0/block/mainnet_genesis.json"))]
#[case::v0_7_0(felt!("0x1"), 2240, test_data("0.7.0/block/mainnet_2240.json"))]
#[case::v0_8_0(felt!("0x1"), 2500, test_data("0.8.0/block/mainnet_2500.json"))]
#[case::v0_9_0(felt!("0x1"), 2800, test_data("0.8.0/block/mainnet_2800.json"))]
#[case::v0_11_1(felt!("0x1"), 65000, test_data("0.11.1/block/mainnet_65000.json"))]
#[tokio::test]
async fn get_class(
    gateway: SequencerGateway,
    #[case] class_hash: ClassHash,
    #[case] block_number: BlockNumber,
    #[case] expected: ContractClass,
) {
    let block_id = BlockIdOrTag::Number(block_number);
    let class = gateway.get_class(class_hash, block_id).await.unwrap();
    similar_asserts::assert_eq!(class, expected);
}

#[rstest]
#[case::pre_v0_7_0(felt!("0x1"), 0, test_data("0.7.0/block/mainnet_genesis.json"))]
#[case::v0_7_0(felt!("0x1"), 2240, test_data("0.7.0/block/mainnet_2240.json"))]
#[case::v0_8_0(felt!("0x1"), 2500, test_data("0.8.0/block/mainnet_2500.json"))]
#[case::v0_9_0(felt!("0x1"), 2800, test_data("0.8.0/block/mainnet_2800.json"))]
#[case::v0_11_1(felt!("0x1"), 65000, test_data("0.11.1/block/mainnet_65000.json"))]
#[tokio::test]
async fn get_compiled_class(
    gateway: SequencerGateway,
    #[case] class_hash: ClassHash,
    #[case] block_number: BlockNumber,
    #[case] expected: CasmContractClass,
) {
    let block_id = BlockIdOrTag::Number(block_number);
    let class = gateway.get_compiled_class(class_hash, block_id).await.unwrap();
    similar_asserts::assert_eq!(class, expected);
}
