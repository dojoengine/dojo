use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use katana_db::mdbx::DbEnv;
use katana_primitives::block::{
    BlockHashOrNumber, BlockIdOrTag, BlockNumber, FinalityStatus, SealedBlockWithStatus,
};
use katana_primitives::hash::{self, StarkHash};
use katana_provider::providers::db::DbProvider;
use katana_provider::providers::fork::ForkedProvider;
use katana_provider::traits::block::{BlockProvider, BlockWriter};
use katana_provider::traits::contract::{ContractClassWriter, ContractClassWriterExt};
use katana_provider::traits::env::BlockEnvProvider;
use katana_provider::traits::stage::StageCheckpointProvider;
use katana_provider::traits::state::{StateFactoryProvider, StateWriter};
use katana_provider::traits::state_update::StateUpdateProvider;
use katana_provider::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider, TransactionTraceProvider,
    TransactionsProviderExt,
};
use katana_provider::traits::trie::TrieWriter;
use katana_provider::BlockchainProvider;
use num_traits::ToPrimitive;
use starknet::core::types::MaybePendingBlockWithTxHashes;
use starknet::core::utils::parse_cairo_short_string;
use starknet::macros::short_string;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use tracing::info;
use url::Url;

pub trait Database:
    BlockProvider
    + BlockWriter
    + TransactionProvider
    + TransactionStatusProvider
    + TransactionTraceProvider
    + TransactionsProviderExt
    + ReceiptProvider
    + StateUpdateProvider
    + StateWriter
    + ContractClassWriter
    + ContractClassWriterExt
    + StateFactoryProvider
    + BlockEnvProvider
    + TrieWriter
    + StageCheckpointProvider
    + 'static
    + Send
    + Sync
    + core::fmt::Debug
{
}

impl<T> Database for T where
    T: BlockProvider
        + BlockWriter
        + TransactionProvider
        + TransactionStatusProvider
        + TransactionTraceProvider
        + TransactionsProviderExt
        + ReceiptProvider
        + StateUpdateProvider
        + StateWriter
        + ContractClassWriter
        + ContractClassWriterExt
        + StateFactoryProvider
        + BlockEnvProvider
        + TrieWriter
        + StageCheckpointProvider
        + 'static
        + Send
        + Sync
        + core::fmt::Debug
{
}

#[derive(Debug, Clone)]
pub struct Blockchain {
    inner: BlockchainProvider<Box<dyn Database>>,
}

impl Blockchain {
    pub fn new(provider: impl Database) -> Self {
        Self { inner: BlockchainProvider::new(Box::new(provider)) }
    }

    /// Creates a new [Blockchain] from a database at `path` and `genesis` state.
    pub fn new_with_db(db: DbEnv) -> Self {
        Self::new(DbProvider::new(db))
    }

    /// Builds a new blockchain with a forked block.
    pub async fn new_from_forked(
        fork_url: Url,
        fork_block: Option<BlockHashOrNumber>,
        chain: &mut katana_chain_spec::dev::ChainSpec,
    ) -> Result<(Self, BlockNumber)> {
        let provider = JsonRpcClient::new(HttpTransport::new(fork_url));
        let chain_id = provider.chain_id().await.context("failed to fetch forked network id")?;

        // if the id is not in ASCII encoding, we display the chain id as is in hex.
        let parsed_id = match parse_cairo_short_string(&chain_id) {
            Ok(id) => id,
            Err(_) => format!("{chain_id:#x}"),
        };

        // If the fork block number is not specified, we use the latest accepted block on the forked
        // network.
        let block_id = if let Some(id) = fork_block {
            id
        } else {
            let num = provider.block_number().await?;
            BlockHashOrNumber::Num(num)
        };

        info!(chain = %parsed_id, block = %block_id, "Forking chain.");

        let block = provider
            .get_block_with_tx_hashes(BlockIdOrTag::from(block_id))
            .await
            .context("failed to fetch forked block")?;

        let MaybePendingBlockWithTxHashes::Block(forked_block) = block else {
            bail!("forking a pending block is not allowed")
        };

        let block_num = forked_block.block_number;

        chain.id = chain_id.into();

        // adjust the genesis to match the forked block
        chain.genesis.timestamp = forked_block.timestamp;
        chain.genesis.number = forked_block.block_number;
        chain.genesis.state_root = forked_block.new_root;
        chain.genesis.parent_hash = forked_block.parent_hash;
        chain.genesis.sequencer_address = forked_block.sequencer_address.into();

        // TODO: remove gas price from genesis
        chain.genesis.gas_prices.eth =
            forked_block.l1_gas_price.price_in_wei.to_u128().expect("should fit in u128");
        chain.genesis.gas_prices.strk =
            forked_block.l1_gas_price.price_in_fri.to_u128().expect("should fit in u128");

        // TODO: convert this to block number instead of BlockHashOrNumber so that it is easier to
        // check if the requested block is within the supported range or not.
        let database = ForkedProvider::new(Arc::new(provider), block_id)?;

        // // update the genesis block with the forked block's data
        // // we dont update the `l1_gas_price` bcs its already done when we set the `gas_prices` in
        // // genesis. this flow is kinda flawed, we should probably refactor it out of the
        // // genesis.
        // let mut block = chain.block();
        // block.header.l1_data_gas_prices.eth =
        //     forked_block.l1_data_gas_price.price_in_wei.to_u128().expect("should fit in u128");
        // block.header.l1_data_gas_prices.strk =
        //     forked_block.l1_data_gas_price.price_in_fri.to_u128().expect("should fit in u128");
        // block.header.l1_da_mode = match forked_block.l1_da_mode {
        //     starknet::core::types::L1DataAvailabilityMode::Blob => L1DataAvailabilityMode::Blob,
        //     starknet::core::types::L1DataAvailabilityMode::Calldata => {
        //         L1DataAvailabilityMode::Calldata
        //     }
        // };

        Ok((Self::new(database), block_num))
    }

    /// Creates a new [Blockchain] with the given [Database] implementation and genesis state.
    pub fn new_dev(
        provider: impl Database,
        chain_spec: &katana_chain_spec::dev::ChainSpec,
    ) -> Result<Self> {
        // check whether the genesis block has been initialized
        let genesis_hash = provider.block_hash_by_num(chain_spec.genesis.number)?;

        match genesis_hash {
            Some(db_hash) => {
                let genesis_hash = chain_spec.block().header.compute_hash();
                // check genesis should be the same
                if db_hash == genesis_hash {
                    Ok(Self::new(provider))
                } else {
                    Err(anyhow!(
                        "Genesis block hash mismatch: expected {genesis_hash:#x}, got {db_hash:#x}",
                    ))
                }
            }

            // Initialize the dev genesis block
            None => {
                let block = chain_spec.block().seal();
                let block = SealedBlockWithStatus { block, status: FinalityStatus::AcceptedOnL1 };
                let states = chain_spec.state_updates();

                let mut block = block;
                let block_number = block.block.header.number;

                let class_trie_root = provider
                    .trie_insert_declared_classes(
                        block_number,
                        &states.state_updates.declared_classes,
                    )
                    .context("failed to update class trie")?;

                let contract_trie_root = provider
                    .trie_insert_contract_updates(block_number, &states.state_updates)
                    .context("failed to update contract trie")?;

                let genesis_state_root = hash::Poseidon::hash_array(&[
                    short_string!("STARKNET_STATE_V0"),
                    contract_trie_root,
                    class_trie_root,
                ]);

                block.block.header.state_root = genesis_state_root;
                provider.insert_block_with_states_and_receipts(block, states, vec![], vec![])?;

                Ok(Self::new(provider))
            }
        }
    }

    pub fn provider(&self) -> &BlockchainProvider<Box<dyn Database>> {
        &self.inner
    }
}
