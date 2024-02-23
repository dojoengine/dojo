use std::sync::Arc;

use katana_executor::ExecutorFactory;
use katana_primitives::block::{
    Block, FinalityStatus, GasPrices, Header, PartialHeader, SealedBlockWithStatus,
};
use katana_primitives::chain::ChainId;
use katana_primitives::env::BlockEnv;
use katana_primitives::receipt::Receipt;
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_primitives::transaction::TxWithHash;
use katana_primitives::version::CURRENT_STARKNET_VERSION;
use katana_primitives::FieldElement;
use katana_provider::providers::fork::ForkedProvider;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_provider::traits::block::{BlockHashProvider, BlockWriter};
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

pub struct Backend<EF: ExecutorFactory> {
    /// The config used to generate the backend.
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
    pub async fn new(executor_factory: Arc<EF>, mut config: StarknetConfig) -> Self {
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
            config.genesis.gas_prices.eth = block.l1_gas_price.price_in_wei;
            config.genesis.gas_prices.strk =
                block.l1_gas_price.price_in_strk.unwrap_or(config.env.gas_price);

            trace!(
                target: "backend",
                "forking chain `{}` at block {} from {}",
                parse_cairo_short_string(&forked_chain_id).unwrap(),
                block.block_number,
                forked_url
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
            Blockchain::new_with_db(db_path, &config.genesis)
                .expect("able to create blockchain from db")
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
        tx_receipt_pairs: Vec<(TxWithHash, Receipt)>,
        state_updates: StateUpdatesWithDeclaredClasses,
    ) -> Result<MinedBlockOutcome, BlockProductionError> {
        let (txs, receipts): (Vec<TxWithHash>, Vec<Receipt>) = tx_receipt_pairs.into_iter().unzip();

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

        let header = Header::new(partial_header, FieldElement::ZERO);
        let block = Block { header, body: txs }.seal();
        let block = SealedBlockWithStatus { block, status: FinalityStatus::AcceptedOnL2 };

        BlockWriter::insert_block_with_states_and_receipts(
            self.blockchain.provider(),
            block,
            state_updates,
            receipts,
        )?;

        info!(target: "backend", "⛏️ Block {block_number} mined with {tx_count} transactions");

        Ok(MinedBlockOutcome { block_number })
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
        self.do_mine_block(block_env, Default::default(), Default::default())
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
        StarknetConfig {
            genesis: Genesis::default(),
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

        assert_eq!(BlockNumberProvider::latest_number(provider).unwrap(), 0);

        let block_num = provider.latest_number().unwrap();
        let mut block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();
        backend.update_block_env(&mut block_env);
        backend.mine_empty_block(&block_env).unwrap();

        let block_num = provider.latest_number().unwrap();
        let mut block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();
        backend.update_block_env(&mut block_env);
        backend.mine_empty_block(&block_env).unwrap();

        let block_num = provider.latest_number().unwrap();
        let block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();

        assert_eq!(BlockNumberProvider::latest_number(provider).unwrap(), 2);
        assert_eq!(block_env.number, 2);

        let block0 = BlockProvider::block_by_number(provider, 0).unwrap().unwrap();
        let block1 = BlockProvider::block_by_number(provider, 1).unwrap().unwrap();
        let block2 = BlockProvider::block_by_number(provider, 2).unwrap().unwrap();

        assert_eq!(block0.header.number, 0);
        assert_eq!(block1.header.number, 1);
        assert_eq!(block2.header.number, 2);
    }
}
