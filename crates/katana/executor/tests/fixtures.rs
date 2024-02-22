use std::collections::HashMap;

use cairo_vm::vm::runners::builtin_runner::{
    BITWISE_BUILTIN_NAME, EC_OP_BUILTIN_NAME, HASH_BUILTIN_NAME, KECCAK_BUILTIN_NAME,
    OUTPUT_BUILTIN_NAME, POSEIDON_BUILTIN_NAME, RANGE_CHECK_BUILTIN_NAME,
    SEGMENT_ARENA_BUILTIN_NAME, SIGNATURE_BUILTIN_NAME,
};
use katana_executor::SimulationFlag;
use katana_primitives::block::{
    Block, ExecutableBlock, FinalityStatus, GasPrices, PartialHeader, SealedBlockWithStatus,
};
use katana_primitives::chain::ChainId;
use katana_primitives::contract::ContractAddress;
use katana_primitives::env::{CfgEnv, FeeTokenAddressses};
use katana_primitives::genesis::allocation::DevAllocationsGenerator;
use katana_primitives::genesis::constant::{
    DEFAULT_FEE_TOKEN_ADDRESS, DEFAULT_PREFUNDED_ACCOUNT_BALANCE,
};
use katana_primitives::genesis::Genesis;
use katana_primitives::transaction::{
    DeployAccountTx, ExecutableTx, ExecutableTxWithHash, InvokeTx,
};
use katana_primitives::version::Version;
use katana_primitives::FieldElement;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_provider::traits::block::BlockWriter;
use katana_provider::traits::state::{StateFactoryProvider, StateProvider};
use starknet::macros::felt;

/// Returns a state provider with some prefilled states.
#[rstest::fixture]
pub fn state_provider() -> Box<dyn StateProvider> {
    let mut seed = [0u8; 32];
    seed[0] = '0' as u8;

    let accounts = DevAllocationsGenerator::new(10)
        .with_seed(seed)
        .with_balance(DEFAULT_PREFUNDED_ACCOUNT_BALANCE)
        .generate();

    let mut genesis = Genesis::default();
    genesis.extend_allocations(accounts.into_iter().map(|(k, v)| (k, v.into())));

    let provider = InMemoryProvider::new();

    let states = genesis.state_updates();
    let block = SealedBlockWithStatus {
        status: FinalityStatus::AcceptedOnL2,
        block: Block::default().seal_with_hash(123u64.into()),
    };

    provider
        .insert_block_with_states_and_receipts(block, states, vec![])
        .expect("able to insert block");

    <InMemoryProvider as StateFactoryProvider>::latest(&provider).unwrap()
}

/// Returns an array of blocks with transaction that are valid against the state by
/// [state_provider].
#[rstest::fixture]
pub fn valid_blocks() -> [ExecutableBlock; 3] {
    let version = Version::new(0, 13, 0);
    let chain_id = ChainId::parse("KATANA").unwrap();
    let sequencer_address = ContractAddress(1u64.into());

    let sender_address = ContractAddress(felt!(
        "0x06b86e40118f29ebe393a75469b4d926c7a44c2e2681b6d319520b7c1156d114"
    ));

    [
        ExecutableBlock {
            header: PartialHeader {
                version,
                number: 1,
                timestamp: 100,
                sequencer_address,
                parent_hash: 123u64.into(),
                gas_prices: GasPrices::default(),
            },
            body: vec![ExecutableTxWithHash::new(ExecutableTx::Invoke(InvokeTx {
                chain_id,
                sender_address,
                calldata: vec![
                    felt!("0x1"),
                    felt!("0x49d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7"),
                    felt!("0x83afd3f4caedc6eebf44246fe54e38c95e3179a5ec9ea81740eca5b482d12e"),
                    felt!("0x3"),
                    felt!("0x77880e2192169bc7107d213ebe643452e1e3e8f40bcc2ebba420b77b1522bd1"),
                    felt!("0x9999999999999"),
                    felt!("0x0"),
                ],
                max_fee: 0,
                version: FieldElement::ONE,
                signature: vec![],
                nonce: 0u64.into(),
            }))],
        },
        ExecutableBlock {
            header: PartialHeader {
                version,
                number: 2,
                timestamp: 200,
                sequencer_address,
                parent_hash: 1234u64.into(),
                gas_prices: GasPrices::default(),
            },
            body: vec![ExecutableTxWithHash::new(ExecutableTx::DeployAccount(DeployAccountTx {
                chain_id,
                max_fee: 0,
                version: FieldElement::ONE,
                signature: vec![],
                nonce: 0u64.into(),
                contract_address_salt: felt!(
                    "0x2ce091f544a799160324295e62da74d194eda204682b5b8fd0dd4d2f8f5ab18"
                ),
                constructor_calldata: vec![felt!(
                    "0x4c339f18b9d1b95b64a6d378abd1480b2e0d5d5bd33cd0828cbce4d65c27284"
                )],
                class_hash: felt!(
                    "0x5400e90f7e0ae78bd02c77cd75527280470e2fe19c54970dd79dc37a9d3645c"
                ),
                contract_address: ContractAddress(felt!(
                    "0x77880e2192169bc7107d213ebe643452e1e3e8f40bcc2ebba420b77b1522bd1"
                )),
            }))],
        },
        ExecutableBlock {
            header: PartialHeader {
                version,
                number: 3,
                timestamp: 300,
                sequencer_address,
                parent_hash: 12345u64.into(),
                gas_prices: GasPrices::default(),
            },
            body: vec![ExecutableTxWithHash::new(ExecutableTx::Invoke(InvokeTx {
                chain_id,
                sender_address,
                calldata: vec![
                    felt!("0x1"),
                    felt!("0x41a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf"),
                    felt!("0x1987cbd17808b9a23693d4de7e246a443cfe37e6e7fbaeabd7d7e6532b07c3d"),
                    felt!("0xa"),
                    felt!("0x2a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f"),
                    felt!("0x6ea2ff5aa6f633708e69f5c61d2ac5f860d2435b46ddbd016aa065bce25100a"),
                    felt!("0x1"),
                    felt!("0x6"),
                    felt!("0x4b415249"),
                    felt!("0x4b415249"),
                    felt!("0x12"),
                    felt!("0x1b39"),
                    felt!("0x0"),
                    felt!("0x6b86e40118f29ebe393a75469b4d926c7a44c2e2681b6d319520b7c1156d114"),
                ],
                max_fee: 0,
                version: FieldElement::ONE,
                signature: vec![],
                nonce: FieldElement::ONE,
            }))],
        },
    ]
}

#[rstest::fixture]
pub fn cfg() -> CfgEnv {
    let fee_token_addresses =
        FeeTokenAddressses { eth: DEFAULT_FEE_TOKEN_ADDRESS, strk: ContractAddress(222u64.into()) };

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
        chain_id: ChainId::parse("KATANA").unwrap(),
    }
}

#[rstest::fixture]
pub fn flags() -> SimulationFlag {
    SimulationFlag {
        skip_validate: true,
        ignore_max_fee: true,
        skip_fee_transfer: true,
        ..Default::default()
    }
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
