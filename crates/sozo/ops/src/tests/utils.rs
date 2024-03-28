use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, TestSequencer,
};
use dojo_world::contracts::world::WorldContract;
use starknet::core::types::FieldElement;

use super::setup;
use crate::utils;

const ACTION_CONTRACT_NAME: &str = "dojo_examples::actions::actions";

#[tokio::test(flavor = "multi_thread")]
async fn get_contract_address_from_world() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    let world = setup::setup(&sequencer).await.unwrap();

    let contract_address =
        utils::get_contract_address(&world, ACTION_CONTRACT_NAME.to_string()).await.unwrap();

    assert!(contract_address != FieldElement::ZERO);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_contract_address_from_string() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    let account = sequencer.account();
    let world = WorldContract::new(FieldElement::ZERO, account);

    let contract_address = utils::get_contract_address(&world, "0x1234".to_string()).await.unwrap();

    assert_eq!(contract_address, FieldElement::from_hex_be("0x1234").unwrap());
}
