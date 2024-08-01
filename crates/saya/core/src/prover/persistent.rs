use serde::{Deserialize, Serialize};
use starknet_crypto::Felt;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BatcherOutput {
    pub prev_state_root: Felt,
    pub new_state_root: Felt,
    pub block_number: Felt,
    pub block_hash: Felt,
    pub config_hash: Felt,
    pub message_to_starknet_segment: Vec<Felt>,
    pub message_to_appchain_segment: Vec<Felt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BatcherCall {
    pub to: Felt,
    pub selector: Felt,
    pub calldata: Vec<Felt>,
    pub starknet_messages: Vec<Felt>,
    pub appchain_messages: Vec<Felt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BatcherInput {
    pub calls: Vec<BatcherCall>,
    pub block_number: Felt,
    pub prev_state_root: Felt,
    pub block_hash: Felt,
}

#[cfg(test)]
mod batcher_args_tests {
    use super::*;
    use cairo_proof_parser::{from_felts, to_felts};

    #[test]
    fn test_batcher_args_no_calls() {
        let no_calls = BatcherInput {
            calls: vec![],
            block_number: Felt::from(1u64),
            prev_state_root: Felt::from(42u64),
            block_hash: Felt::from(52u64),
        };

        let serialized = to_felts(&no_calls).unwrap();
        let expected = [0u64, 1, 42, 52].into_iter().map(Felt::from).collect::<Vec<_>>();
        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_batcher_args_single_call() {
        let no_calls = BatcherInput {
            calls: vec![BatcherCall {
                to: Felt::from(1u64),
                selector: Felt::from(2u64),
                calldata: vec![Felt::from(3u64), Felt::from(4u64)],
                starknet_messages: Vec::new(),
                appchain_messages: Vec::new(),
            }],
            block_number: Felt::from(1u64),
            prev_state_root: Felt::from(42u64),
            block_hash: Felt::from(52u64),
        };

        let serialized = to_felts(&no_calls).unwrap();
        let expected =
            [1u64, 1, 2, 2, 3, 4, 0, 0, 1, 42, 52].into_iter().map(Felt::from).collect::<Vec<_>>();
        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_parse_program_output() {
        let output = [0, 1, 2, 0x34, 0x2a, 0, 0u64].into_iter().map(Felt::from).collect::<Vec<_>>();

        let parsed = from_felts::<BatcherOutput>(&output).unwrap();
        let expected = BatcherOutput {
            prev_state_root: Felt::from(0u64),
            new_state_root: Felt::from(1u64),
            block_number: Felt::from(2u64),
            block_hash: Felt::from(52u64),
            config_hash: Felt::from(42u64),
            message_to_starknet_segment: vec![],
            message_to_appchain_segment: vec![],
        };

        assert_eq!(parsed, expected);
    }
}
