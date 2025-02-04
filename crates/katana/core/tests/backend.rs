use alloy_primitives::U256;
use katana_chain_spec::rollup::{self, FeeContract};
use katana_chain_spec::{dev, ChainSpec, SettlementLayer};
use katana_core::backend::gas_oracle::GasOracle;
use katana_core::backend::storage::{Blockchain, Database};
use katana_core::backend::Backend;
use katana_executor::implementation::blockifier::BlockifierFactory;
use katana_primitives::chain::ChainId;
use katana_primitives::env::CfgEnv;
use katana_primitives::felt;
use katana_primitives::genesis::allocation::DevAllocationsGenerator;
use katana_primitives::genesis::constant::DEFAULT_PREFUNDED_ACCOUNT_BALANCE;
use katana_primitives::genesis::Genesis;
use katana_provider::providers::db::DbProvider;
use rstest::rstest;
use url::Url;

fn executor(chain_spec: &ChainSpec) -> BlockifierFactory {
    BlockifierFactory::new(
        CfgEnv {
            chain_id: chain_spec.id(),
            validate_max_n_steps: u32::MAX,
            invoke_tx_max_n_steps: u32::MAX,
            max_recursion_depth: usize::MAX,
            ..Default::default()
        },
        Default::default(),
    )
}

fn backend(chain_spec: &ChainSpec) -> Backend<BlockifierFactory> {
    backend_with_db(chain_spec, DbProvider::new_ephemeral())
}

fn backend_with_db(chain_spec: &ChainSpec, provider: impl Database) -> Backend<BlockifierFactory> {
    Backend::new(
        chain_spec.clone().into(),
        Blockchain::new(provider),
        GasOracle::sampled_starknet(),
        executor(chain_spec),
    )
}

fn dev_chain_spec() -> dev::ChainSpec {
    dev::ChainSpec::default()
}

fn rollup_chain_spec() -> rollup::ChainSpec {
    let accounts = DevAllocationsGenerator::new(10)
        .with_balance(U256::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE))
        .generate();

    let mut genesis = Genesis::default();
    genesis.extend_allocations(accounts.into_iter().map(|(k, v)| (k, v.into())));

    let id = ChainId::parse("KATANA").unwrap();
    let fee_contract = FeeContract::default();

    let settlement = SettlementLayer::Starknet {
        id: ChainId::default(),
        account: Default::default(),
        core_contract: Default::default(),
        rpc_url: Url::parse("http://localhost:5050").unwrap(),
    };

    rollup::ChainSpec { id, genesis, settlement, fee_contract }
}

#[rstest]
#[case::dev(ChainSpec::Dev(dev_chain_spec()))]
#[case::rollup(ChainSpec::Rollup(rollup_chain_spec()))]
fn can_initialize_genesis(#[case] chain: ChainSpec) {
    let backend = backend(&chain);
    backend.init_genesis().expect("failed to initialize genesis");
}

#[test]
fn reinitialize_with_different_rollup_chain_spec() {
    let db = DbProvider::new_ephemeral();

    let chain1 = ChainSpec::Rollup(rollup_chain_spec());
    let backend1 = backend_with_db(&chain1, db.clone());
    backend1.init_genesis().expect("failed to initialize genesis");

    // Modify the chain spec so that the resultant genesis block hash will be different.
    let chain2 = ChainSpec::Rollup({
        let mut chain = rollup_chain_spec();
        chain.genesis.parent_hash = felt!("0x1337");
        chain
    });

    let backend2 = backend_with_db(&chain2, db);
    let err = backend2.init_genesis().unwrap_err().to_string();
    assert!(err.as_str().contains("Genesis block hash mismatch"));
}
