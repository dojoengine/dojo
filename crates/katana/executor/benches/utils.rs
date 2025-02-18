use std::sync::Arc;

use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_utils::TransactionWaiter;
use katana_chain_spec::ChainSpec;
use katana_node::config::db::DbConfig;
use katana_node::config::dev::DevConfig;
use katana_node::Node;
use katana_primitives::block::GasPrices;
use katana_primitives::chain::ChainId;
use katana_primitives::contract::Nonce;
use katana_primitives::da::DataAvailabilityMode;
use katana_primitives::env::{BlockEnv, CfgEnv, FeeTokenAddressses};
use katana_primitives::fee::ResourceBoundsMapping;
use katana_primitives::genesis::constant::DEFAULT_ETH_FEE_TOKEN_ADDRESS;
use katana_primitives::transaction::{
    ExecutableTx, ExecutableTxWithHash, InvokeTx, InvokeTxV1, InvokeTxV3,
};
use katana_primitives::Felt;
use scarb::compiler::Profile;
use starknet::accounts::{
    Account, ConnectedAccount, ExecutionEncoder, ExecutionEncoding, SingleOwnerAccount,
};
use starknet::macros::{felt, selector};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Url};
use starknet::signers::{LocalWallet, SigningKey};

pub fn tx() -> ExecutableTxWithHash {
    let invoke = InvokeTx::V1(InvokeTxV1 {
        sender_address: felt!("0x1").into(),
        calldata: vec![
            DEFAULT_ETH_FEE_TOKEN_ADDRESS.into(),
            selector!("transfer"),
            Felt::THREE,
            felt!("0x100"),
            Felt::ONE,
            Felt::ZERO,
        ],
        max_fee: 10_000,
        ..Default::default()
    });

    ExecutableTxWithHash::new(invoke.into())
}

pub fn envs() -> (BlockEnv, CfgEnv) {
    (block_env(), cfg_env())
}

pub fn block_env() -> BlockEnv {
    BlockEnv {
        l1_gas_prices: GasPrices { eth: 1, strk: 1 },
        sequencer_address: felt!("0x1337").into(),
        ..Default::default()
    }
}

pub fn cfg_env() -> CfgEnv {
    CfgEnv {
        max_recursion_depth: 100,
        validate_max_n_steps: 4_000_000,
        invoke_tx_max_n_steps: 4_000_000,
        fee_token_addresses: FeeTokenAddressses {
            eth: DEFAULT_ETH_FEE_TOKEN_ADDRESS,
            strk: DEFAULT_ETH_FEE_TOKEN_ADDRESS,
        },
        ..Default::default()
    }
}

#[allow(unused)]
pub fn setup() -> (Node, MoveTxGenerator) {
    use dojo_test_utils::migration::copy_spawn_and_move_db;
    use dojo_world::contracts::naming::compute_selector_from_names;
    use katana_node::config::Config;
    use sozo_scarbext::WorkspaceExt;
    use tokio::runtime::Builder;

    Builder::new_multi_thread().enable_all().build().unwrap().block_on(async {
        let config = Config {
            db: DbConfig { dir: Some(copy_spawn_and_move_db().into_std_path_buf()) },
            dev: DevConfig { fee: false, account_validation: false, ..Default::default() },
            chain: Arc::new(ChainSpec::dev()),
            ..Default::default()
        };

        let handle = katana_node::build(config)
            .await
            .expect("failed to build node")
            .launch()
            .await
            .expect("failed to launch node");

        let addr = handle.rpc.addr();
        let url = Url::parse(&format!("http://{}", addr)).unwrap();
        let provider = JsonRpcClient::new(HttpTransport::new(url));

        let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
        let config = setup.build_test_config("spawn-and-move", Profile::DEV);

        let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

        let world_local = ws.load_world_local().unwrap();
        let actions_address = world_local
            .get_contract_address_local(compute_selector_from_names("ns", "actions"))
            .unwrap();

        let (addr, ..) = handle.node.backend.chain_spec.genesis().accounts().next().unwrap();
        let chain_id = handle.node.backend.chain_spec.id();

        let account = SingleOwnerAccount::new(
            provider,
            LocalWallet::from_signing_key(SigningKey::from_random()),
            (*addr).into(),
            chain_id.into(),
            ExecutionEncoding::New,
        );

        let contract = abigen::SpawnAndMoveAction::new(actions_address, &account);
        let res = contract.spawn().send().await.unwrap();
        TransactionWaiter::new(res.transaction_hash, account.provider()).await.unwrap();

        let initial_nonce = account.get_nonce().await.expect("failed to get initial nonce");
        let tx_generator = MoveTxGenerator::new(account, chain_id, initial_nonce, actions_address);

        let provider = handle.node.backend.blockchain.provider().clone();

        (handle.node, tx_generator)
    })
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct MoveTxGenerator {
    chain_id: ChainId,
    nonce: Nonce,
    actions_address: Felt,
    account: Arc<SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>>,
}

impl MoveTxGenerator {
    fn new(
        account: SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
        chain_id: ChainId,
        initial_nonce: Nonce,
        actions_address: Felt,
    ) -> Self {
        Self { account: account.into(), chain_id, nonce: initial_nonce, actions_address }
    }

    #[allow(unused)]
    pub fn move_tx(&mut self) -> ExecutableTxWithHash {
        let contract = abigen::SpawnAndMoveAction::new(self.actions_address, &self.account);
        let call = contract.move_getcall(&abigen::Direction::None);

        let calldata = self.account.encode_calls(&[call]);
        let transaction = ExecutableTx::Invoke(InvokeTx::V3(InvokeTxV3 {
            tip: 0,
            nonce: self.nonce,
            calldata,
            chain_id: self.chain_id,
            signature: Default::default(),
            paymaster_data: Default::default(),
            sender_address: self.account.address().into(),
            account_deployment_data: Default::default(),
            resource_bounds: ResourceBoundsMapping::default(),
            fee_data_availability_mode: DataAvailabilityMode::L1,
            nonce_data_availability_mode: DataAvailabilityMode::L1,
        }));
        let hash = transaction.calculate_hash(false);

        self.nonce += Felt::ONE;

        ExecutableTxWithHash { hash, transaction }
    }
}

// Uncomment this once this PR is merged: https://github.com/cartridge-gg/cainome/pull/80

// cainome::rs::abigen!(SpawnAndMoveAction, [
//     {"type":"enum","name":"dojo_examples::models::Direction","variants":[{"name":"None","type":"
// ()"},{"name":"Left","type":"()"},{"name":"Right","type":"()"},{"name":"Up","type":"()"},{"name":"
// Down","type":"()"}]},     {"type":"function","name":"spawn","inputs":[],"outputs":[],"
// state_mutability":"external"},     {"type":"function","name":"move","inputs":[{"name":"direction"
// ,"type":"dojo_examples::models::Direction"}],"outputs":[],"state_mutability":"external"} ]);

// Generated by `abigen!`
#[allow(unused)]
mod abigen {

    pub struct SpawnAndMoveAction<A: starknet::accounts::ConnectedAccount + Sync> {
        pub address: starknet::core::types::Felt,
        pub account: A,
        pub block_id: starknet::core::types::BlockId,
    }

    impl<A: starknet::accounts::ConnectedAccount + Sync> SpawnAndMoveAction<A> {
        pub fn new(address: starknet::core::types::Felt, account: A) -> Self {
            Self {
                address,
                account,
                block_id: starknet::core::types::BlockId::Tag(
                    starknet::core::types::BlockTag::Pending,
                ),
            }
        }

        pub fn set_contract_address(&mut self, address: starknet::core::types::Felt) {
            self.address = address;
        }

        pub fn provider(&self) -> &A::Provider {
            self.account.provider()
        }

        pub fn set_block(&mut self, block_id: starknet::core::types::BlockId) {
            self.block_id = block_id;
        }

        pub fn with_block(self, block_id: starknet::core::types::BlockId) -> Self {
            Self { block_id, ..self }
        }
    }

    #[derive()]
    pub struct SpawnAndMoveActionReader<P: starknet::providers::Provider + Sync> {
        pub address: starknet::core::types::Felt,
        pub provider: P,
        pub block_id: starknet::core::types::BlockId,
    }

    impl<P: starknet::providers::Provider + Sync> SpawnAndMoveActionReader<P> {
        pub fn new(address: starknet::core::types::Felt, provider: P) -> Self {
            Self {
                address,
                provider,
                block_id: starknet::core::types::BlockId::Tag(
                    starknet::core::types::BlockTag::Pending,
                ),
            }
        }

        pub fn set_contract_address(&mut self, address: starknet::core::types::Felt) {
            self.address = address;
        }

        pub fn provider(&self) -> &P {
            &self.provider
        }

        pub fn set_block(&mut self, block_id: starknet::core::types::BlockId) {
            self.block_id = block_id;
        }

        pub fn with_block(self, block_id: starknet::core::types::BlockId) -> Self {
            Self { block_id, ..self }
        }
    }
    #[derive()]
    pub enum Direction {
        None,
        Left,
        Right,
        Up,
        Down,
    }

    impl cainome::cairo_serde::CairoSerde for Direction {
        type RustType = Self;
        const SERIALIZED_SIZE: std::option::Option<usize> = std::option::Option::None;
        #[inline]
        fn cairo_serialized_size(__rust: &Self::RustType) -> usize {
            match __rust {
                Direction::None => 1,
                Direction::Left => 1,
                Direction::Right => 1,
                Direction::Up => 1,
                Direction::Down => 1,
                _ => 0,
            }
        }
        fn cairo_serialize(__rust: &Self::RustType) -> Vec<starknet::core::types::Felt> {
            match __rust {
                Direction::None => usize::cairo_serialize(&0usize),
                Direction::Left => usize::cairo_serialize(&1usize),
                Direction::Right => usize::cairo_serialize(&2usize),
                Direction::Up => usize::cairo_serialize(&3usize),
                Direction::Down => usize::cairo_serialize(&4usize),
                _ => Vec::new(),
            }
        }
        fn cairo_deserialize(
            __felts: &[starknet::core::types::Felt],
            __offset: usize,
        ) -> cainome::cairo_serde::Result<Self::RustType> {
            let __f = __felts[__offset];
            let __index = u128::from_be_bytes(__f.to_bytes_be()[16..].try_into().unwrap());
            match __index as usize {
                0usize => Ok(Direction::None),
                1usize => Ok(Direction::Left),
                2usize => Ok(Direction::Right),
                3usize => Ok(Direction::Up),
                4usize => Ok(Direction::Down),
                _ => Err(cainome::cairo_serde::Error::Deserialize(format!(
                    "Index not handle for enum {}",
                    "Direction"
                ))),
            }
        }
    }
    impl<A: starknet::accounts::ConnectedAccount + Sync> SpawnAndMoveAction<A> {
        #[allow(clippy::ptr_arg)]
        #[allow(clippy::too_many_arguments)]
        pub fn move_getcall(&self, direction: &Direction) -> starknet::core::types::Call {
            use cainome::cairo_serde::CairoSerde;
            let mut __calldata = Vec::new();
            __calldata.extend(Direction::cairo_serialize(direction));
            starknet::core::types::Call {
                to: self.address,
                selector: ::starknet::core::types::Felt::from_raw([
                    67542746491835804,
                    12863064398400549321,
                    17438469324404153095,
                    10377031306845665431,
                ]),
                calldata: __calldata,
            }
        }

        #[allow(clippy::ptr_arg)]
        #[allow(clippy::too_many_arguments)]
        pub fn r#move(&self, direction: &Direction) -> starknet::accounts::ExecutionV1<'_, A> {
            use cainome::cairo_serde::CairoSerde;
            let mut __calldata = Vec::new();
            __calldata.extend(Direction::cairo_serialize(direction));
            let __call = starknet::core::types::Call {
                to: self.address,
                selector: ::starknet::core::types::Felt::from_raw([
                    67542746491835804,
                    12863064398400549321,
                    17438469324404153095,
                    10377031306845665431,
                ]),
                calldata: __calldata,
            };
            self.account.execute_v1(vec![__call])
        }

        #[allow(clippy::ptr_arg)]
        #[allow(clippy::too_many_arguments)]
        pub fn spawn(&self) -> starknet::accounts::ExecutionV1<'_, A> {
            let mut __calldata = Vec::new();
            let __call = starknet::core::types::Call {
                to: self.address,
                selector: ::starknet::core::types::Felt::from_raw([
                    427316234342132431,
                    10119134573058282481,
                    17664319446359752539,
                    173372654641669380,
                ]),
                calldata: __calldata,
            };
            self.account.execute_v1(vec![__call])
        }
    }

    impl<P: starknet::providers::Provider + Sync> SpawnAndMoveActionReader<P> {}
}
