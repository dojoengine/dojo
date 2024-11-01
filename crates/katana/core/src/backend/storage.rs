use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use katana_db::mdbx::DbEnv;
use katana_primitives::block::{
    BlockHashOrNumber, BlockIdOrTag, BlockNumber, FinalityStatus, SealedBlockWithStatus,
};
use katana_primitives::chain_spec::ChainSpec;
use katana_primitives::da::L1DataAvailabilityMode;
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_primitives::version::ProtocolVersion;
use katana_provider::providers::db::DbProvider;
use katana_provider::providers::fork::ForkedProvider;
use katana_provider::traits::block::{BlockProvider, BlockWriter};
use katana_provider::traits::contract::ContractClassWriter;
use katana_provider::traits::env::BlockEnvProvider;
use katana_provider::traits::state::{StateFactoryProvider, StateRootProvider, StateWriter};
use katana_provider::traits::state_update::StateUpdateProvider;
use katana_provider::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider, TransactionTraceProvider,
    TransactionsProviderExt,
};
use katana_provider::traits::trie::{ClassTrieWriter, ContractTrieWriter};
use katana_provider::BlockchainProvider;
use num_traits::ToPrimitive;
use starknet::core::types::{BlockStatus, MaybePendingBlockWithTxHashes};
use starknet::core::utils::parse_cairo_short_string;
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
    + StateRootProvider
    + StateWriter
    + ContractClassWriter
    + StateFactoryProvider
    + BlockEnvProvider
    + ClassTrieWriter
    + ContractTrieWriter
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
        + StateRootProvider
        + StateWriter
        + ContractClassWriter
        + StateFactoryProvider
        + BlockEnvProvider
        + ClassTrieWriter
        + ContractTrieWriter
        + 'static
        + Send
        + Sync
        + core::fmt::Debug
{
}

#[derive(Debug)]
pub struct Blockchain {
    inner: BlockchainProvider<Box<dyn Database>>,
}

impl Blockchain {
    pub fn new(provider: impl Database) -> Self {
        Self { inner: BlockchainProvider::new(Box::new(provider)) }
    }

    /// Creates a new [Blockchain] with the given [Database] implementation and genesis state.
    pub fn new_with_chain(provider: impl Database, chain: &ChainSpec) -> Result<Self> {
        // check whether the genesis block has been initialized
        let genesis_hash = provider.block_hash_by_num(chain.genesis.number)?;

        match genesis_hash {
            Some(db_hash) => {
                let genesis_hash = chain.block().header.compute_hash();
                // check genesis should be the same
                if db_hash == genesis_hash {
                    Ok(Self::new(provider))
                } else {
                    Err(anyhow!(
                        "Genesis block hash mismatch: expected {genesis_hash:#x}, got {db_hash:#}",
                    ))
                }
            }

            None => {
                let block = chain.block().seal();
                let block = SealedBlockWithStatus { block, status: FinalityStatus::AcceptedOnL1 };
                let state_updates = chain.state_updates();

                Self::new_with_genesis_block_and_state(provider, block, state_updates)
            }
        }
    }

    /// Creates a new [Blockchain] from a database at `path` and `genesis` state.
    pub fn new_with_db(db: DbEnv, chain: &ChainSpec) -> Result<Self> {
        Self::new_with_chain(DbProvider::new(db), chain)
    }

    /// Builds a new blockchain with a forked block.
    pub async fn new_from_forked(
        fork_url: Url,
        fork_block: Option<BlockHashOrNumber>,
        chain: &mut ChainSpec,
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
        chain.version = ProtocolVersion::parse(&forked_block.starknet_version)?;

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

        let status = match forked_block.status {
            BlockStatus::AcceptedOnL1 => FinalityStatus::AcceptedOnL1,
            BlockStatus::AcceptedOnL2 => FinalityStatus::AcceptedOnL2,
            // we already checked for pending block earlier. so this should never happen.
            _ => bail!("qed; block status shouldn't be pending"),
        };

        // TODO: convert this to block number instead of BlockHashOrNumber so that it is easier to
        // check if the requested block is within the supported range or not.
        let database = ForkedProvider::new(Arc::new(provider), block_id)?;

        // update the genesis block with the forked block's data
        // we dont update the `l1_gas_price` bcs its already done when we set the `gas_prices` in
        // genesis. this flow is kinda flawed, we should probably refactor it out of the
        // genesis.
        let mut block = chain.block();
        block.header.l1_data_gas_prices.eth =
            forked_block.l1_data_gas_price.price_in_wei.to_u128().expect("should fit in u128");
        block.header.l1_data_gas_prices.strk =
            forked_block.l1_data_gas_price.price_in_fri.to_u128().expect("should fit in u128");
        block.header.l1_da_mode = match forked_block.l1_da_mode {
            starknet::core::types::L1DataAvailabilityMode::Blob => L1DataAvailabilityMode::Blob,
            starknet::core::types::L1DataAvailabilityMode::Calldata => {
                L1DataAvailabilityMode::Calldata
            }
        };

        let block = block.seal_with_hash_and_status(forked_block.block_hash, status);
        let state_updates = chain.state_updates();

        let blockchain = Self::new_with_genesis_block_and_state(database, block, state_updates)?;
        Ok((blockchain, block_num))
    }

    pub fn provider(&self) -> &BlockchainProvider<Box<dyn Database>> {
        &self.inner
    }

    fn new_with_genesis_block_and_state(
        provider: impl Database,
        block: SealedBlockWithStatus,
        states: StateUpdatesWithDeclaredClasses,
    ) -> Result<Self> {
        BlockWriter::insert_block_with_states_and_receipts(
            &provider,
            block,
            states,
            vec![],
            vec![],
        )?;
        Ok(Self::new(provider))
    }
}

#[cfg(test)]
mod tests {
    use katana_primitives::block::{
        Block, FinalityStatus, GasPrices, Header, SealedBlockWithStatus,
    };
    use katana_primitives::da::L1DataAvailabilityMode;
    use katana_primitives::fee::{PriceUnit, TxFeeInfo};
    use katana_primitives::genesis::constant::{
        DEFAULT_ETH_FEE_TOKEN_ADDRESS, DEFAULT_LEGACY_ERC20_CASM, DEFAULT_LEGACY_ERC20_CLASS_HASH,
        DEFAULT_LEGACY_UDC_CASM, DEFAULT_LEGACY_UDC_CLASS_HASH, DEFAULT_UDC_ADDRESS,
    };
    use katana_primitives::receipt::{InvokeTxReceipt, Receipt};
    use katana_primitives::state::StateUpdatesWithDeclaredClasses;
    use katana_primitives::trace::TxExecInfo;
    use katana_primitives::transaction::{InvokeTx, Tx, TxWithHash};
    use katana_primitives::{chain_spec, Felt};
    use katana_provider::providers::db::DbProvider;
    use katana_provider::traits::block::{
        BlockHashProvider, BlockNumberProvider, BlockProvider, BlockWriter,
    };
    use katana_provider::traits::state::StateFactoryProvider;
    use katana_provider::traits::transaction::{TransactionProvider, TransactionTraceProvider};
    use starknet::macros::felt;

    use super::Blockchain;

    #[test]
    fn blockchain_from_genesis_states() {
        let provider = DbProvider::new_ephemeral();

        let blockchain = Blockchain::new_with_chain(provider, &chain_spec::DEV)
            .expect("failed to create blockchain from genesis block");
        let state = blockchain.provider().latest().expect("failed to get latest state");

        let latest_number = blockchain.provider().latest_number().unwrap();
        let fee_token_class_hash =
            state.class_hash_of_contract(DEFAULT_ETH_FEE_TOKEN_ADDRESS).unwrap().unwrap();
        let udc_class_hash = state.class_hash_of_contract(DEFAULT_UDC_ADDRESS).unwrap().unwrap();

        assert_eq!(latest_number, 0);
        assert_eq!(udc_class_hash, DEFAULT_LEGACY_UDC_CLASS_HASH);
        assert_eq!(fee_token_class_hash, DEFAULT_LEGACY_ERC20_CLASS_HASH);
    }

    #[test]
    fn blockchain_from_db() {
        let db_path = tempfile::TempDir::new().expect("Failed to create temp dir.").into_path();

        let dummy_tx = TxWithHash {
            hash: felt!("0xbad"),
            transaction: Tx::Invoke(InvokeTx::V1(Default::default())),
        };

        let dummy_block = SealedBlockWithStatus {
            status: FinalityStatus::AcceptedOnL1,
            block: Block {
                header: Header {
                    parent_hash: Felt::ZERO,
                    number: 1,
                    l1_gas_prices: GasPrices::default(),
                    l1_data_gas_prices: GasPrices::default(),
                    l1_da_mode: L1DataAvailabilityMode::Calldata,
                    timestamp: 123456,
                    ..Default::default()
                },
                body: vec![dummy_tx.clone()],
            }
            .seal(),
        };

        {
            let db = katana_db::init_db(&db_path).expect("Failed to init database");
            let blockchain = Blockchain::new_with_db(db, &chain_spec::DEV)
                .expect("Failed to create db-backed blockchain storage");

            blockchain
                .provider()
                .insert_block_with_states_and_receipts(
                    dummy_block.clone(),
                    StateUpdatesWithDeclaredClasses::default(),
                    vec![Receipt::Invoke(InvokeTxReceipt {
                        revert_error: None,
                        events: Vec::new(),
                        messages_sent: Vec::new(),
                        execution_resources: Default::default(),
                        fee: TxFeeInfo {
                            gas_price: 0,
                            overall_fee: 0,
                            gas_consumed: 0,
                            unit: PriceUnit::Wei,
                        },
                    })],
                    vec![TxExecInfo::default()],
                )
                .unwrap();

            // assert genesis state is correct

            let state = blockchain.provider().latest().expect("failed to get latest state");

            let actual_udc_class_hash =
                state.class_hash_of_contract(DEFAULT_UDC_ADDRESS).unwrap().unwrap();
            let actual_udc_class = state.class(actual_udc_class_hash).unwrap().unwrap();

            let actual_fee_token_class_hash =
                state.class_hash_of_contract(DEFAULT_ETH_FEE_TOKEN_ADDRESS).unwrap().unwrap();
            let actual_fee_token_class = state.class(actual_fee_token_class_hash).unwrap().unwrap();

            assert_eq!(actual_udc_class_hash, DEFAULT_LEGACY_UDC_CLASS_HASH);
            assert_eq!(actual_udc_class, DEFAULT_LEGACY_UDC_CASM.clone());

            assert_eq!(actual_fee_token_class_hash, DEFAULT_LEGACY_ERC20_CLASS_HASH);
            assert_eq!(actual_fee_token_class, DEFAULT_LEGACY_ERC20_CASM.clone());
        }

        // re open the db and assert the state is the same and not overwritten

        {
            let db = katana_db::init_db(db_path).expect("Failed to init database");
            let blockchain = Blockchain::new_with_db(db, &chain_spec::DEV)
                .expect("Failed to create db-backed blockchain storage");

            // assert genesis state is correct

            let state = blockchain.provider().latest().expect("failed to get latest state");

            let actual_udc_class_hash =
                state.class_hash_of_contract(DEFAULT_UDC_ADDRESS).unwrap().unwrap();
            let actual_udc_class = state.class(actual_udc_class_hash).unwrap().unwrap();

            let actual_fee_token_class_hash =
                state.class_hash_of_contract(DEFAULT_ETH_FEE_TOKEN_ADDRESS).unwrap().unwrap();
            let actual_fee_token_class = state.class(actual_fee_token_class_hash).unwrap().unwrap();

            assert_eq!(actual_udc_class_hash, DEFAULT_LEGACY_UDC_CLASS_HASH);
            assert_eq!(actual_udc_class, DEFAULT_LEGACY_UDC_CASM.clone());

            assert_eq!(actual_fee_token_class_hash, DEFAULT_LEGACY_ERC20_CLASS_HASH);
            assert_eq!(actual_fee_token_class, DEFAULT_LEGACY_ERC20_CASM.clone());

            let block_number = blockchain.provider().latest_number().unwrap();
            let block_hash = blockchain.provider().latest_hash().unwrap();
            let block =
                blockchain.provider().block_by_hash(dummy_block.block.hash).unwrap().unwrap();

            let tx = blockchain.provider().transaction_by_hash(dummy_tx.hash).unwrap().unwrap();
            let tx_exec =
                blockchain.provider().transaction_execution(dummy_tx.hash).unwrap().unwrap();

            assert_eq!(block_hash, dummy_block.block.hash);
            assert_eq!(block_number, dummy_block.block.header.number);
            assert_eq!(block, dummy_block.block.unseal());
            assert_eq!(tx, dummy_tx);
            assert_eq!(tx_exec, TxExecInfo::default());
        }
    }
}
