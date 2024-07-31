use cairo_proof_parser::{from_felts, to_felts};
use serde::{Deserialize, Serialize};
use starknet_crypto::FieldElement;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BatcherOutput {
    pub prev_state_root: FieldElement,
    pub new_state_root: FieldElement,
    pub block_number: FieldElement,
    pub block_hash: FieldElement,
    pub config_hash: FieldElement,
    pub message_to_starknet_segment: Vec<FieldElement>,
    pub message_to_appchain_segment: Vec<FieldElement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BatcherCall {
    pub to: FieldElement,
    pub selector: FieldElement,
    pub calldata: Vec<FieldElement>,
    pub starknet_messages: Vec<FieldElement>,
    pub appchain_messages: Vec<FieldElement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BatcherInput {
    pub calls: Vec<BatcherCall>,
    pub block_number: FieldElement,
    pub prev_state_root: FieldElement,
    pub block_hash: FieldElement,
}

#[test]
fn test_batcher_args_no_calls() {
    let no_calls = BatcherInput {
        calls: vec![],
        block_number: FieldElement::from(1u64),
        prev_state_root: FieldElement::from(42u64),
        block_hash: FieldElement::from(52u64),
    };

    let serialized = to_felts(&no_calls).unwrap();
    let expected = [0u64, 1, 42, 52].into_iter().map(FieldElement::from).collect::<Vec<_>>();
    assert_eq!(serialized, expected);
}

#[test]
fn test_batcher_args_single_call() {
    let no_calls = BatcherInput {
        calls: vec![BatcherCall {
            to: FieldElement::from(1u64),
            selector: FieldElement::from(2u64),
            calldata: vec![FieldElement::from(3u64), FieldElement::from(4u64)],
            starknet_messages: Vec::new(),
            appchain_messages: Vec::new(),
        }],
        block_number: FieldElement::from(1u64),
        prev_state_root: FieldElement::from(42u64),
        block_hash: FieldElement::from(52u64),
    };

    let serialized = to_felts(&no_calls).unwrap();
    let expected = [1u64, 1, 2, 2, 3, 4, 0, 0, 1, 42, 52]
        .into_iter()
        .map(FieldElement::from)
        .collect::<Vec<_>>();
    assert_eq!(serialized, expected);
}

#[test]
fn test_parse_program_output() {
    let output =
        [0, 1, 2, 0x34, 0x2a, 0, 0u64].into_iter().map(FieldElement::from).collect::<Vec<_>>();

    let parsed = from_felts::<BatcherOutput>(&output).unwrap();
    let expected = BatcherOutput {
        prev_state_root: FieldElement::from(0u64),
        new_state_root: FieldElement::from(1u64),
        block_number: FieldElement::from(2u64),
        block_hash: FieldElement::from(52u64),
        config_hash: FieldElement::from(42u64),
        message_to_starknet_segment: vec![],
        message_to_appchain_segment: vec![],
    };

    assert_eq!(parsed, expected);
}
