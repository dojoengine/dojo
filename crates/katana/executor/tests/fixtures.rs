use std::collections::HashMap;

use cairo_vm::vm::runners::builtin_runner::{
    BITWISE_BUILTIN_NAME, EC_OP_BUILTIN_NAME, HASH_BUILTIN_NAME, KECCAK_BUILTIN_NAME,
    OUTPUT_BUILTIN_NAME, POSEIDON_BUILTIN_NAME, RANGE_CHECK_BUILTIN_NAME,
    SEGMENT_ARENA_BUILTIN_NAME, SIGNATURE_BUILTIN_NAME,
};
use katana_executor::SimulationFlag;
use katana_primitives::block::{Block, ExecutableBlock, FinalityStatus, SealedBlockWithStatus};
use katana_primitives::chain::ChainId;
use katana_primitives::contract::ContractAddress;
use katana_primitives::env::{CfgEnv, FeeTokenAddressses};
use katana_primitives::genesis::Genesis;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_provider::traits::block::BlockWriter;
use katana_provider::traits::state::{StateFactoryProvider, StateProvider};

/// Returns a state provider with some prefilled states.
#[rstest::fixture]
pub fn state_provider() -> Box<dyn StateProvider> {
    let states = Genesis::default().state_updates();
    let provider = InMemoryProvider::new();

    let block = SealedBlockWithStatus {
        status: FinalityStatus::AcceptedOnL2,
        block: Block::default().seal_with_hash(123u64.into()),
    };

    provider
        .insert_block_with_states_and_receipts(block, states, vec![])
        .expect("able to insert block");

    <InMemoryProvider as StateFactoryProvider>::latest(&provider).unwrap()
}

#[rstest::fixture]
pub fn valid_blocks() -> [ExecutableBlock; 3] {
    todo!()
}

#[rstest::fixture]
pub fn cfg() -> CfgEnv {
    let fee_token_addresses = FeeTokenAddressses {
        eth: ContractAddress(111u64.into()),
        strk: ContractAddress(222u64.into()),
    };

    let vm_resource_fee_cost = HashMap::from([
        (String::from("n_steps"), 1_f64),
        (HASH_BUILTIN_NAME.to_string(), 1_f64),
        (RANGE_CHECK_BUILTIN_NAME.to_string(), 1_f64),
        (SIGNATURE_BUILTIN_NAME.to_string(), 1_f64),
        (BITWISE_BUILTIN_NAME.to_string(), 1_f64),
        (POSEIDON_BUILTIN_NAME.to_string(), 1_f64),
        (OUTPUT_BUILTIN_NAME.to_string(), 1_f64),
        (EC_OP_BUILTIN_NAME.to_string(), 1_f64),
        (KECCAK_BUILTIN_NAME.to_string(), 1_f64),
        (SEGMENT_ARENA_BUILTIN_NAME.to_string(), 1_f64),
    ]);

    CfgEnv {
        fee_token_addresses,
        vm_resource_fee_cost,
        max_recursion_depth: 100,
        validate_max_n_steps: 1_000_000,
        invoke_tx_max_n_steps: 1_000_000,
        chain_id: ChainId::MAINNET,
    }
}

#[rstest::fixture]
pub fn flags() -> SimulationFlag {
    todo!()
}

pub mod blockifier {
    use katana_executor::implementation::blockifier::BlockifierFactory;
    use katana_executor::SimulationFlag;

    use super::{cfg, flags, CfgEnv};

    #[rstest::fixture]
    pub fn factory(cfg: CfgEnv, flags: SimulationFlag) -> BlockifierFactory {
        BlockifierFactory::new(cfg, flags)
    }
}
