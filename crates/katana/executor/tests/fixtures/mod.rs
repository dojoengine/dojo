pub mod transaction;

use std::collections::HashMap;

use cairo_vm::vm::runners::builtin_runner::{
    BITWISE_BUILTIN_NAME, EC_OP_BUILTIN_NAME, HASH_BUILTIN_NAME, KECCAK_BUILTIN_NAME,
    OUTPUT_BUILTIN_NAME, POSEIDON_BUILTIN_NAME, RANGE_CHECK_BUILTIN_NAME,
    SEGMENT_ARENA_BUILTIN_NAME, SIGNATURE_BUILTIN_NAME,
};
use katana_executor::implementation::noop::NoopExecutorFactory;
use katana_executor::{ExecutorFactory, SimulationFlag};
use katana_primitives::block::{
    Block, ExecutableBlock, FinalityStatus, GasPrices, PartialHeader, SealedBlockWithStatus,
};
use katana_primitives::chain::ChainId;
use katana_primitives::class::{CompiledClass, FlattenedSierraClass};
use katana_primitives::contract::ContractAddress;
use katana_primitives::env::{CfgEnv, FeeTokenAddressses};
use katana_primitives::genesis::allocation::DevAllocationsGenerator;
use katana_primitives::genesis::constant::{
    DEFAULT_FEE_TOKEN_ADDRESS, DEFAULT_PREFUNDED_ACCOUNT_BALANCE,
};
use katana_primitives::genesis::Genesis;
use katana_primitives::transaction::{
    DeclareTx, DeclareTxV2, DeclareTxWithClass, DeployAccountTx, DeployAccountTxV1, ExecutableTx,
    ExecutableTxWithHash, InvokeTx, InvokeTxV1,
};
use katana_primitives::utils::class::{parse_compiled_class, parse_sierra_class};
use katana_primitives::version::Version;
use katana_primitives::FieldElement;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_provider::traits::block::BlockWriter;
use katana_provider::traits::state::{StateFactoryProvider, StateProvider};
use starknet::macros::felt;

// TODO: remove support for legacy contract declaration
#[allow(unused)]
pub fn legacy_contract_class() -> CompiledClass {
    let json = include_str!("legacy_contract.json");
    let artifact = serde_json::from_str(json).unwrap();
    parse_compiled_class(artifact).unwrap()
}

pub fn contract_class() -> (CompiledClass, FlattenedSierraClass) {
    let json = include_str!("contract.json");
    let artifact = serde_json::from_str(json).unwrap();

    let sierra = parse_sierra_class(json).unwrap().flatten().unwrap();
    let compiled = parse_compiled_class(artifact).unwrap();

    (compiled, sierra)
}

#[rstest::fixture]
#[once]
pub fn genesis() -> Genesis {
    let mut seed = [0u8; 32];
    seed[0] = b'0';

    let accounts = DevAllocationsGenerator::new(10)
        .with_seed(seed)
        .with_balance(DEFAULT_PREFUNDED_ACCOUNT_BALANCE)
        .generate();

    let mut genesis = Genesis::default();
    genesis.extend_allocations(accounts.into_iter().map(|(k, v)| (k, v.into())));
    genesis
}

/// Returns a state provider with some prefilled states.
#[rstest::fixture]
pub fn state_provider(genesis: &Genesis) -> Box<dyn StateProvider> {
    let states = genesis.state_updates();
    let provider = InMemoryProvider::new();

    let block = SealedBlockWithStatus {
        status: FinalityStatus::AcceptedOnL2,
        block: Block::default().seal_with_hash(123u64.into()),
    };

    provider
        .insert_block_with_states_and_receipts(block, states, vec![], vec![])
        .expect("able to insert block");

    <InMemoryProvider as StateFactoryProvider>::latest(&provider).unwrap()
}

// TODO: update the txs to include valid signatures
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

    let gas_prices = GasPrices { eth: 100 * u128::pow(10, 9), strk: 100 * u128::pow(10, 9) };

    [
        ExecutableBlock {
            header: PartialHeader {
                version,
                number: 1,
                timestamp: 100,
                sequencer_address,
                parent_hash: 123u64.into(),
                gas_prices: gas_prices.clone(),
            },
            body: vec![
                // fund the account to be deployed, sending 0x9999999999999 amount
                ExecutableTxWithHash::new(ExecutableTx::Invoke(InvokeTx::V1(InvokeTxV1 {
                    chain_id,
                    sender_address,
                    calldata: vec![
                        felt!("0x1"),
                        felt!("0x49d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7"),
                        felt!("0x83afd3f4caedc6eebf44246fe54e38c95e3179a5ec9ea81740eca5b482d12e"),
                        felt!("0x3"),
                        felt!("0x3ddfa445a70b927497249f94ff7431fc2e2abc761a34417fd4891beb7c2db85"),
                        felt!("0x9999999999999999"),
                        felt!("0x0"),
                    ],
                    max_fee: 4367000000000000,
                    signature: vec![],
                    nonce: FieldElement::ZERO,
                }))),
                // declare contract
                ExecutableTxWithHash::new(ExecutableTx::Declare({
                    let (compiled_class, sierra) = contract_class();
                    DeclareTxWithClass {
                        compiled_class,
                        sierra_class: Some(sierra),
                        transaction: DeclareTx::V2(DeclareTxV2 {
                            nonce: FieldElement::ONE,
                            max_fee: 982300000000000,
                            chain_id,
                            signature: vec![],
                            sender_address,
                            class_hash: felt!("0x420"),
                            compiled_class_hash: felt!(
                                "0x16c6081eb34ad1e0c5513234ed0c025b3c7f305902d291bad534cd6474c85bc"
                            ),
                        }),
                    }
                })),
            ],
        },
        ExecutableBlock {
            header: PartialHeader {
                version,
                number: 2,
                timestamp: 200,
                sequencer_address,
                parent_hash: 1234u64.into(),
                gas_prices: gas_prices.clone(),
            },
            body: vec![
                // deploy account tx with the default account class
                ExecutableTxWithHash::new(ExecutableTx::DeployAccount(DeployAccountTx::V1(
                    DeployAccountTxV1 {
                        chain_id,
                        max_fee: 1443900000000000,
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
                            "0x3ddfa445a70b927497249f94ff7431fc2e2abc761a34417fd4891beb7c2db85"
                        )),
                    },
                ))),
            ],
        },
        ExecutableBlock {
            header: PartialHeader {
                version,
                number: 3,
                timestamp: 300,
                sequencer_address,
                parent_hash: 12345u64.into(),
                gas_prices: gas_prices.clone(),
            },
            body: vec![
                // deploy contract using UDC
                ExecutableTxWithHash::new(ExecutableTx::Invoke(InvokeTx::V1(InvokeTxV1 {
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
                        sender_address.into(),
                    ],
                    max_fee: 2700700000000000,
                    signature: vec![],
                    nonce: FieldElement::TWO,
                }))),
            ],
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

// TODO: test both with and without the flags turned on
#[rstest::fixture]
pub fn flags(
    #[default(false)] skip_validate: bool,
    #[default(false)] skip_fee_transfer: bool,
) -> SimulationFlag {
    SimulationFlag { skip_validate, skip_fee_transfer, ..Default::default() }
}

/// A fixture that provides a default `ExecutorFactory` implementation.
#[rstest::fixture]
#[default(NoopExecutorFactory)]
pub fn executor_factory<EF: ExecutorFactory>(
    #[default(NoopExecutorFactory::new())] factory: EF,
) -> EF {
    factory
}

#[cfg(feature = "blockifier")]
pub mod blockifier {
    use katana_executor::implementation::blockifier::BlockifierFactory;
    use katana_executor::SimulationFlag;

    use super::{cfg, flags, CfgEnv};

    #[rstest::fixture]
    pub fn factory(cfg: CfgEnv, #[with(true)] flags: SimulationFlag) -> BlockifierFactory {
        BlockifierFactory::new(cfg, flags)
    }
}

#[cfg(feature = "sir")]
pub mod sir {
    use katana_executor::implementation::sir::NativeExecutorFactory;
    use katana_executor::SimulationFlag;

    use super::{cfg, flags, CfgEnv};

    #[rstest::fixture]
    pub fn factory(cfg: CfgEnv, #[with(true)] flags: SimulationFlag) -> NativeExecutorFactory {
        NativeExecutorFactory::new(cfg, flags)
    }
}
