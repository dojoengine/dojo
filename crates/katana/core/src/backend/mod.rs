use std::sync::Arc;

use katana_executor::{ExecutionOutput, ExecutionResult, ExecutorFactory};
use katana_primitives::block::{
    Block, FinalityStatus, GasPrices, Header, PartialHeader, SealedBlockWithStatus,
};
use katana_primitives::chain::ChainId;
use katana_primitives::env::BlockEnv;
use katana_primitives::transaction::TxHash;
use katana_primitives::version::CURRENT_STARKNET_VERSION;
use katana_primitives::Felt;
use katana_provider::providers::fork::ForkedProvider;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_provider::traits::block::{BlockHashProvider, BlockWriter};
use num_traits::ToPrimitive;
use parking_lot::RwLock;
use starknet::core::types::{BlockId, BlockStatus, MaybePendingBlockWithTxHashes};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use tracing::{info, trace};

pub mod config;
pub mod contract;
pub mod storage;

use self::config::StarknetConfig;
use self::storage::Blockchain;
use crate::env::BlockContextGenerator;
use crate::service::block_producer::{BlockProductionError, MinedBlockOutcome};
use crate::utils::get_current_timestamp;

pub(crate) const LOG_TARGET: &str = "katana::core::backend";

#[derive(Debug)]
pub struct Backend<EF: ExecutorFactory> {
    /// The config used to generate the backend.
    #[deprecated]
    pub config: StarknetConfig,
    /// stores all block related data in memory
    pub blockchain: Blockchain,
    /// The chain id.
    pub chain_id: ChainId,
    /// The block context generator.
    pub block_context_generator: RwLock<BlockContextGenerator>,

    pub executor_factory: Arc<EF>,
}

impl<EF: ExecutorFactory> Backend<EF> {
    #[allow(deprecated, unused)]
    pub(crate) async fn new(executor_factory: Arc<EF>, mut config: StarknetConfig) -> Self {
        let block_context_generator = config.block_context_generator();

        let blockchain: Blockchain = if let Some(forked_url) = &config.fork_rpc_url {
            let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(forked_url.clone())));
            let forked_chain_id = provider.chain_id().await.unwrap();

            let forked_block_num = if let Some(num) = config.fork_block_number {
                num
            } else {
                provider
                    .block_number()
                    .await
                    .expect("failed to fetch block number from forked network")
            };

            let block =
                provider.get_block_with_tx_hashes(BlockId::Number(forked_block_num)).await.unwrap();
            let MaybePendingBlockWithTxHashes::Block(block) = block else {
                panic!("block to be forked is a pending block")
            };

            // adjust the genesis to match the forked block
            config.genesis.number = block.block_number;
            config.genesis.state_root = block.new_root;
            config.genesis.parent_hash = block.parent_hash;
            config.genesis.timestamp = block.timestamp;
            config.genesis.sequencer_address = block.sequencer_address.into();
            config.genesis.gas_prices.eth =
                block.l1_gas_price.price_in_wei.to_u128().expect("should fit in u128");
            config.genesis.gas_prices.strk =
                block.l1_gas_price.price_in_fri.to_u128().expect("should fit in u128");

            trace!(
                target: LOG_TARGET,
                chain = %parse_cairo_short_string(&forked_chain_id).unwrap(),
                block_number = %block.block_number,
                forked_url = %forked_url,
                "Forking chain.",
            );

            let blockchain = Blockchain::new_from_forked(
                ForkedProvider::new(provider, forked_block_num.into()).unwrap(),
                block.block_hash,
                &config.genesis,
                match block.status {
                    BlockStatus::AcceptedOnL1 => FinalityStatus::AcceptedOnL1,
                    BlockStatus::AcceptedOnL2 => FinalityStatus::AcceptedOnL2,
                    _ => panic!("unable to fork for non-accepted block"),
                },
            )
            .expect("able to create forked blockchain");

            config.env.chain_id = forked_chain_id.into();
            blockchain
        } else if let Some(db_path) = &config.db_dir {
            let db = katana_db::init_db(db_path).expect("failed to initialize db");
            Blockchain::new_with_db(db, &config.genesis).expect("able to create blockchain from db")
        } else {
            Blockchain::new_with_genesis(InMemoryProvider::new(), &config.genesis)
                .expect("able to create blockchain from genesis block")
        };

        Self {
            chain_id: config.env.chain_id,
            blockchain,
            config,
            executor_factory,
            block_context_generator: RwLock::new(block_context_generator),
        }
    }

    pub fn do_mine_block(
        &self,
        block_env: &BlockEnv,
        execution_output: ExecutionOutput,
    ) -> Result<MinedBlockOutcome, BlockProductionError> {
        // we optimistically allocate the maximum amount possible
        let mut txs = Vec::with_capacity(execution_output.transactions.len());
        let mut traces = Vec::with_capacity(execution_output.transactions.len());
        let mut receipts = Vec::with_capacity(execution_output.transactions.len());

        // only include successful transactions in the block
        for (tx, res) in execution_output.transactions {
            if let ExecutionResult::Success { receipt, trace, .. } = res {
                txs.push(tx);
                traces.push(trace);
                receipts.push(receipt);
            }
        }

        let prev_hash = BlockHashProvider::latest_hash(self.blockchain.provider())?;
        let block_number = block_env.number;
        let tx_count = txs.len();

        let partial_header = PartialHeader {
            number: block_number,
            parent_hash: prev_hash,
            version: CURRENT_STARKNET_VERSION,
            timestamp: block_env.timestamp,
            sequencer_address: block_env.sequencer_address,
            gas_prices: GasPrices {
                eth: block_env.l1_gas_prices.eth,
                strk: block_env.l1_gas_prices.strk,
            },
        };

        let tx_hashes = txs.iter().map(|tx| tx.hash).collect::<Vec<TxHash>>();
        let header = Header::new(partial_header, Felt::ZERO);
        let block = Block { header, body: txs }.seal();
        let block = SealedBlockWithStatus { block, status: FinalityStatus::AcceptedOnL2 };

        BlockWriter::insert_block_with_states_and_receipts(
            self.blockchain.provider(),
            block,
            execution_output.states,
            receipts,
            traces,
        )?;

        info!(
            target: LOG_TARGET,
            block_number = %block_number,
            tx_count = %tx_count,
            "Block mined.",
        );

        Ok(MinedBlockOutcome { block_number, txs: tx_hashes, stats: execution_output.stats })
    }

    pub fn update_block_env(&self, block_env: &mut BlockEnv) {
        let mut context_gen = self.block_context_generator.write();
        let current_timestamp_secs = get_current_timestamp().as_secs() as i64;

        let timestamp = if context_gen.next_block_start_time == 0 {
            (current_timestamp_secs + context_gen.block_timestamp_offset) as u64
        } else {
            let timestamp = context_gen.next_block_start_time;
            context_gen.block_timestamp_offset = timestamp as i64 - current_timestamp_secs;
            context_gen.next_block_start_time = 0;
            timestamp
        };

        block_env.number += 1;
        block_env.timestamp = timestamp;
    }

    pub fn mine_empty_block(
        &self,
        block_env: &BlockEnv,
    ) -> Result<MinedBlockOutcome, BlockProductionError> {
        self.do_mine_block(block_env, Default::default())
    }
}

#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use katana_executor::implementation::noop::NoopExecutorFactory;
    use katana_primitives::genesis::Genesis;
    use katana_provider::traits::block::{BlockNumberProvider, BlockProvider};
    use katana_provider::traits::env::BlockEnvProvider;

    use super::Backend;
    use crate::backend::config::{Environment, StarknetConfig};

    fn create_test_starknet_config() -> StarknetConfig {
        let mut genesis = Genesis::default();
        genesis.gas_prices.eth = 2100;
        genesis.gas_prices.strk = 3100;

        StarknetConfig {
            genesis,
            disable_fee: true,
            env: Environment::default(),
            ..Default::default()
        }
    }

    async fn create_test_backend() -> Backend<NoopExecutorFactory> {
        Backend::new(Arc::new(NoopExecutorFactory::default()), create_test_starknet_config()).await
    }

    #[tokio::test]
    async fn test_creating_blocks() {
        let backend = create_test_backend().await;
        let provider = backend.blockchain.provider();

        let block_num = provider.latest_number().unwrap();
        let block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();

        assert_eq!(block_num, 0);
        assert_eq!(block_env.number, 0);
        assert_eq!(block_env.l1_gas_prices.eth, 2100);
        assert_eq!(block_env.l1_gas_prices.strk, 3100);

        let mut block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();
        backend.update_block_env(&mut block_env);
        backend.mine_empty_block(&block_env).unwrap();

        let block_num = provider.latest_number().unwrap();
        assert_eq!(block_num, 1);
        assert_eq!(block_env.number, 1);
        assert_eq!(block_env.l1_gas_prices.eth, 2100);
        assert_eq!(block_env.l1_gas_prices.strk, 3100);

        let mut block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();
        backend.update_block_env(&mut block_env);
        backend.mine_empty_block(&block_env).unwrap();

        let block_num = provider.latest_number().unwrap();
        let block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();

        let block_num = provider.latest_number().unwrap();
        assert_eq!(block_num, 2);
        assert_eq!(block_env.number, 2);
        assert_eq!(block_env.l1_gas_prices.eth, 2100);
        assert_eq!(block_env.l1_gas_prices.strk, 3100);

        let block0 = BlockProvider::block_by_number(provider, 0).unwrap().unwrap();
        let block1 = BlockProvider::block_by_number(provider, 1).unwrap().unwrap();
        let block2 = BlockProvider::block_by_number(provider, 2).unwrap().unwrap();

        assert_eq!(block0.header.number, 0);
        assert_eq!(block1.header.number, 1);
        assert_eq!(block2.header.number, 2);
    }
}
