use std::sync::Arc;

use blockifier::block_context::BlockContext;
use katana_primitives::block::{
    Block, FinalityStatus, GasPrices, Header, PartialHeader, SealedBlockWithStatus,
};
use katana_primitives::contract::ContractAddress;
use katana_primitives::receipt::Receipt;
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_primitives::transaction::TxWithHash;
use katana_primitives::version::CURRENT_STARKNET_VERSION;
use katana_primitives::FieldElement;
use katana_provider::providers::fork::ForkedProvider;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_provider::traits::block::{BlockHashProvider, BlockWriter};
use katana_provider::traits::state::{StateFactoryProvider, StateProvider};
use parking_lot::RwLock;
use starknet::core::types::{BlockId, BlockStatus, MaybePendingBlockWithTxHashes};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use starknet_api::block::{BlockNumber, BlockTimestamp};
use starknet_api::core::ChainId;
use tracing::{info, trace};

pub mod config;
pub mod contract;
pub mod storage;

use self::config::StarknetConfig;
use self::storage::Blockchain;
use crate::accounts::{Account, DevAccountGenerator};
use crate::constants::DEFAULT_PREFUNDED_ACCOUNT_BALANCE;
use crate::env::{BlockContextGenerator, Env};
use crate::service::block_producer::MinedBlockOutcome;
use crate::utils::get_current_timestamp;

pub struct Backend {
    /// The config used to generate the backend.
    pub config: RwLock<StarknetConfig>,
    /// stores all block related data in memory
    pub blockchain: Blockchain,
    /// The chain environment values.
    pub env: Arc<RwLock<Env>>,
    pub block_context_generator: RwLock<BlockContextGenerator>,
    /// Prefunded dev accounts
    pub accounts: Vec<Account>,
}

impl Backend {
    pub async fn new(config: StarknetConfig) -> Self {
        let mut block_context = config.block_context();
        let block_context_generator = config.block_context_generator();

        let accounts = DevAccountGenerator::new(config.total_accounts)
            .with_seed(config.seed)
            .with_balance(*DEFAULT_PREFUNDED_ACCOUNT_BALANCE)
            .generate();

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

            block_context.block_number = BlockNumber(block.block_number);
            block_context.block_timestamp = BlockTimestamp(block.timestamp);
            block_context.sequencer_address = ContractAddress(block.sequencer_address).into();
            block_context.chain_id = ChainId(parse_cairo_short_string(&forked_chain_id).unwrap());

            trace!(
                target: "backend",
                "forking chain `{}` at block {} from {}",
                parse_cairo_short_string(&forked_chain_id).unwrap(),
                block.block_number,
                forked_url
            );

            Blockchain::new_from_forked(
                ForkedProvider::new(provider, forked_block_num.into()),
                block.block_hash,
                block.parent_hash,
                &block_context,
                block.new_root,
                match block.status {
                    BlockStatus::AcceptedOnL1 => FinalityStatus::AcceptedOnL1,
                    BlockStatus::AcceptedOnL2 => FinalityStatus::AcceptedOnL2,
                    _ => panic!("unable to fork for non-accepted block"),
                },
            )
            .expect("able to create forked blockchain")
        } else {
            Blockchain::new_with_genesis(InMemoryProvider::new(), &block_context)
                .expect("able to create blockchain from genesis block")
        };

        let env = Env { block: block_context };

        for acc in &accounts {
            acc.deploy_and_fund(blockchain.provider())
                .expect("should be able to deploy and fund dev account");
        }

        Self {
            accounts,
            blockchain,
            config: RwLock::new(config),
            env: Arc::new(RwLock::new(env)),
            block_context_generator: RwLock::new(block_context_generator),
        }
    }

    /// Mines a new block based on the provided execution outcome.
    /// This method should only be called by the
    /// [IntervalBlockProducer](crate::service::block_producer::IntervalBlockProducer) when the node
    /// is running in `interval` mining mode.
    pub fn mine_pending_block(
        &self,
        tx_receipt_pairs: Vec<(TxWithHash, Receipt)>,
        state_updates: StateUpdatesWithDeclaredClasses,
    ) -> (MinedBlockOutcome, Box<dyn StateProvider>) {
        let block_context = self.env.read().block.clone();
        let outcome = self.do_mine_block(block_context, tx_receipt_pairs, state_updates);
        let new_state = StateFactoryProvider::latest(&self.blockchain.provider()).unwrap();
        (outcome, new_state)
    }

    pub fn do_mine_block(
        &self,
        block_context: BlockContext,
        tx_receipt_pairs: Vec<(TxWithHash, Receipt)>,
        state_updates: StateUpdatesWithDeclaredClasses,
    ) -> MinedBlockOutcome {
        let (txs, receipts): (Vec<TxWithHash>, Vec<Receipt>) = tx_receipt_pairs.into_iter().unzip();

        let prev_hash = BlockHashProvider::latest_hash(self.blockchain.provider()).unwrap();

        let partial_header = PartialHeader {
            parent_hash: prev_hash,
            version: CURRENT_STARKNET_VERSION,
            timestamp: block_context.block_timestamp.0,
            sequencer_address: block_context.sequencer_address.into(),
            gas_prices: GasPrices {
                eth_gas_price: block_context.gas_prices.eth_l1_gas_price.try_into().unwrap(),
                strk_gas_price: block_context.gas_prices.strk_l1_gas_price.try_into().unwrap(),
            },
        };

        let tx_count = txs.len();
        let block_number = block_context.block_number.0;

        let header = Header::new(partial_header, block_number, FieldElement::ZERO);
        let block = Block { header, body: txs }.seal();
        let block = SealedBlockWithStatus { block, status: FinalityStatus::AcceptedOnL2 };

        BlockWriter::insert_block_with_states_and_receipts(
            self.blockchain.provider(),
            block,
            state_updates,
            receipts,
        )
        .unwrap();

        info!(target: "backend", "⛏️ Block {block_number} mined with {tx_count} transactions");

        MinedBlockOutcome { block_number }
    }

    pub fn update_block_context(&self) {
        let mut context_gen = self.block_context_generator.write();
        let block_context = &mut self.env.write().block;
        let current_timestamp_secs = get_current_timestamp().as_secs() as i64;

        let timestamp = if context_gen.next_block_start_time == 0 {
            (current_timestamp_secs + context_gen.block_timestamp_offset) as u64
        } else {
            let timestamp = context_gen.next_block_start_time;
            context_gen.block_timestamp_offset = timestamp as i64 - current_timestamp_secs;
            context_gen.next_block_start_time = 0;
            timestamp
        };

        block_context.block_number = block_context.block_number.next();
        block_context.block_timestamp = BlockTimestamp(timestamp);
    }

    /// Updates the block context and mines an empty block.
    pub fn mine_empty_block(&self) -> MinedBlockOutcome {
        self.update_block_context();
        let block_context = self.env.read().block.clone();
        self.do_mine_block(block_context, Default::default(), Default::default())
    }
}
