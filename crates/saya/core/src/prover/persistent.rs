use cairo_proof_parser::to_felts;
use serde::{Deserialize, Serialize};
use starknet_crypto::FieldElement;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BatcherOutput {
    prev_state_root: FieldElement,
    new_state_root: FieldElement,
    block_number: FieldElement,
    block_hash: FieldElement,
    config_hash: FieldElement,
    message_to_starknet_segment: Vec<FieldElement>,
    message_to_appchain_segment: Vec<FieldElement>,
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
