use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use starknet_crypto::Felt;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BatcherOutput {
    pub padding: [Felt; 2],
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

#[derive(Debug, Serialize, Deserialize)]
pub struct StarknetOsOutput {
    /// The root before.
    pub initial_root: Felt,
    /// The root after.
    pub final_root: Felt,
    /// The previous block number.
    pub prev_block_number: Felt,
    /// The current block number.
    pub new_block_number: Felt,
    /// The previous block hash.
    pub prev_block_hash: Felt,
    /// The current block hash.
    pub new_block_hash: Felt,
    /// The hash of the OS program, if the aggregator was used. Zero if the OS was used directly.
    pub os_program_hash: Felt,
    /// The hash of the OS config.
    pub starknet_os_config_hash: Felt,
    /// Whether KZG data availability was used.
    pub use_kzg_da: Felt,
    /// Indicates whether previous state values are included in the state update information.
    pub full_output: Felt,
    /// Messages from L2 to L1.
    pub messages_to_l1: Vec<Felt>,
    /// Messages from L1 to L2.
    pub messages_to_l2: Vec<Felt>,
    /// The list of contracts that were changed.
    pub contracts: Vec<ContractChanges>,
    /// The list of classes that were declared. A map from class hash to compiled class hash.
    pub classes: HashMap<Felt, Felt>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ContractChanges {
    /// The address of the contract.
    pub addr: Felt,
    /// The new nonce of the contract (for account contracts).
    pub nonce: Felt,
    /// The new class hash (if changed).
    pub class_hash: Option<Felt>,
    /// A map from storage key to its new value.
    pub storage_changes: HashMap<Felt, Felt>,
}

#[cfg(test)]
mod batcher_args_tests {
    use cairo_proof_parser::{from_felts, to_felts};

    use super::*;

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
        let output =
            [0, 0, 0, 1, 2, 0x34, 0x2a, 0, 0u64].into_iter().map(Felt::from).collect::<Vec<_>>();

        let parsed = from_felts::<BatcherOutput>(&output).unwrap();
        let expected = BatcherOutput {
            padding: [Felt::from(0u64); 2],
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
