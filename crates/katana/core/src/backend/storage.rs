use anyhow::{anyhow, Result};
use katana_db::mdbx::DbEnv;
use katana_primitives::block::{BlockHash, FinalityStatus, SealedBlockWithStatus};
use katana_primitives::genesis::Genesis;
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_provider::providers::db::DbProvider;
use katana_provider::traits::block::{BlockProvider, BlockWriter};
use katana_provider::traits::contract::ContractClassWriter;
use katana_provider::traits::env::BlockEnvProvider;
use katana_provider::traits::messaging::MessagingProvider;
use katana_provider::traits::state::{StateFactoryProvider, StateRootProvider, StateWriter};
use katana_provider::traits::state_update::StateUpdateProvider;
use katana_provider::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider, TransactionTraceProvider,
    TransactionsProviderExt,
};
use katana_provider::BlockchainProvider;

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
    + MessagingProvider
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
        + MessagingProvider
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
    pub fn new_with_genesis(provider: impl Database, genesis: &Genesis) -> Result<Self> {
        // check whether the genesis block has been initialized
        let genesis_hash = provider.block_hash_by_num(genesis.number)?;

        match genesis_hash {
            Some(db_hash) => {
                let genesis_hash = genesis.block().header.compute_hash();
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
                let block = genesis.block().seal();
                let block = SealedBlockWithStatus { block, status: FinalityStatus::AcceptedOnL1 };
                let state_updates = genesis.state_updates();

                Self::new_with_block_and_state(provider, block, state_updates)
            }
        }
    }

    /// Creates a new [Blockchain] from a database at `path` and `genesis` state.
    pub fn new_with_db(db: DbEnv, genesis: &Genesis) -> Result<Self> {
        Self::new_with_genesis(DbProvider::new(db), genesis)
    }

    /// Builds a new blockchain with a forked block.
    pub fn new_from_forked(
        provider: impl Database,
        genesis_hash: BlockHash,
        genesis: &Genesis,
        block_status: FinalityStatus,
    ) -> Result<Self> {
        let block = genesis.block().seal_with_hash_and_status(genesis_hash, block_status);
        let state_updates = genesis.state_updates();
        Self::new_with_block_and_state(provider, block, state_updates)
    }

    pub fn provider(&self) -> &BlockchainProvider<Box<dyn Database>> {
        &self.inner
    }

    fn new_with_block_and_state(
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
    use katana_primitives::fee::TxFeeInfo;
    use katana_primitives::genesis::constant::{
        DEFAULT_FEE_TOKEN_ADDRESS, DEFAULT_LEGACY_ERC20_CONTRACT_CASM,
        DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH, DEFAULT_LEGACY_UDC_CASM,
        DEFAULT_LEGACY_UDC_CLASS_HASH, DEFAULT_UDC_ADDRESS,
    };
    use katana_primitives::genesis::Genesis;
    use katana_primitives::receipt::{InvokeTxReceipt, Receipt};
    use katana_primitives::state::StateUpdatesWithDeclaredClasses;
    use katana_primitives::trace::TxExecInfo;
    use katana_primitives::transaction::{InvokeTx, Tx, TxWithHash};
    use katana_primitives::Felt;
    use katana_provider::providers::in_memory::InMemoryProvider;
    use katana_provider::traits::block::{
        BlockHashProvider, BlockNumberProvider, BlockProvider, BlockStatusProvider, BlockWriter,
        HeaderProvider,
    };
    use katana_provider::traits::state::StateFactoryProvider;
    use katana_provider::traits::transaction::{TransactionProvider, TransactionTraceProvider};
    use starknet::core::types::PriceUnit;
    use starknet::macros::felt;

    use super::Blockchain;

    #[test]
    fn blockchain_from_genesis_states() {
        let provider = InMemoryProvider::new();

        let blockchain = Blockchain::new_with_genesis(provider, &Genesis::default())
            .expect("failed to create blockchain from genesis block");
        let state = blockchain.provider().latest().expect("failed to get latest state");

        let latest_number = blockchain.provider().latest_number().unwrap();
        let fee_token_class_hash =
            state.class_hash_of_contract(DEFAULT_FEE_TOKEN_ADDRESS).unwrap().unwrap();
        let udc_class_hash = state.class_hash_of_contract(DEFAULT_UDC_ADDRESS).unwrap().unwrap();

        assert_eq!(latest_number, 0);
        assert_eq!(udc_class_hash, DEFAULT_LEGACY_UDC_CLASS_HASH);
        assert_eq!(fee_token_class_hash, DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH);
    }

    #[test]
    fn blockchain_from_fork() {
        let provider = InMemoryProvider::new();

        let genesis = Genesis {
            number: 23,
            parent_hash: Felt::ZERO,
            state_root: felt!("1334"),
            timestamp: 6868,
            gas_prices: GasPrices { eth: 9090, strk: 8080 },
            ..Default::default()
        };

        let genesis_hash = felt!("1111");

        let blockchain = Blockchain::new_from_forked(
            provider,
            genesis_hash,
            &genesis,
            FinalityStatus::AcceptedOnL1,
        )
        .expect("failed to create fork blockchain");

        let latest_number = blockchain.provider().latest_number().unwrap();
        let latest_hash = blockchain.provider().latest_hash().unwrap();
        let header = blockchain.provider().header(latest_number.into()).unwrap().unwrap();
        let block_status =
            blockchain.provider().block_status(latest_number.into()).unwrap().unwrap();

        assert_eq!(latest_number, genesis.number);
        assert_eq!(latest_hash, genesis_hash);

        assert_eq!(header.gas_prices.eth, 9090);
        assert_eq!(header.gas_prices.strk, 8080);
        assert_eq!(header.timestamp, 6868);
        assert_eq!(header.number, latest_number);
        assert_eq!(header.state_root, genesis.state_root);
        assert_eq!(header.parent_hash, genesis.parent_hash);
        assert_eq!(block_status, FinalityStatus::AcceptedOnL1);
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
                    gas_prices: GasPrices::default(),
                    timestamp: 123456,
                    ..Default::default()
                },
                body: vec![dummy_tx.clone()],
            }
            .seal(),
        };

        let genesis = Genesis::default();

        {
            let db = katana_db::init_db(&db_path).expect("Failed to init database");
            let blockchain = Blockchain::new_with_db(db, &genesis)
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
                state.class_hash_of_contract(DEFAULT_FEE_TOKEN_ADDRESS).unwrap().unwrap();
            let actual_fee_token_class = state.class(actual_fee_token_class_hash).unwrap().unwrap();

            assert_eq!(actual_udc_class_hash, DEFAULT_LEGACY_UDC_CLASS_HASH);
            assert_eq!(actual_udc_class, DEFAULT_LEGACY_UDC_CASM.clone());

            assert_eq!(actual_fee_token_class_hash, DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH);
            assert_eq!(actual_fee_token_class, DEFAULT_LEGACY_ERC20_CONTRACT_CASM.clone());
        }

        // re open the db and assert the state is the same and not overwritten

        {
            let db = katana_db::init_db(db_path).expect("Failed to init database");
            let blockchain = Blockchain::new_with_db(db, &genesis)
                .expect("Failed to create db-backed blockchain storage");

            // assert genesis state is correct

            let state = blockchain.provider().latest().expect("failed to get latest state");

            let actual_udc_class_hash =
                state.class_hash_of_contract(DEFAULT_UDC_ADDRESS).unwrap().unwrap();
            let actual_udc_class = state.class(actual_udc_class_hash).unwrap().unwrap();

            let actual_fee_token_class_hash =
                state.class_hash_of_contract(DEFAULT_FEE_TOKEN_ADDRESS).unwrap().unwrap();
            let actual_fee_token_class = state.class(actual_fee_token_class_hash).unwrap().unwrap();

            assert_eq!(actual_udc_class_hash, DEFAULT_LEGACY_UDC_CLASS_HASH);
            assert_eq!(actual_udc_class, DEFAULT_LEGACY_UDC_CASM.clone());

            assert_eq!(actual_fee_token_class_hash, DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH);
            assert_eq!(actual_fee_token_class, DEFAULT_LEGACY_ERC20_CONTRACT_CASM.clone());

            let block_number = blockchain.provider().latest_number().unwrap();
            let block_hash = blockchain.provider().latest_hash().unwrap();
            let block = blockchain
                .provider()
                .block_by_hash(dummy_block.block.header.hash)
                .unwrap()
                .unwrap();

            let tx = blockchain.provider().transaction_by_hash(dummy_tx.hash).unwrap().unwrap();
            let tx_exec =
                blockchain.provider().transaction_execution(dummy_tx.hash).unwrap().unwrap();

            assert_eq!(block_hash, dummy_block.block.header.hash);
            assert_eq!(block_number, dummy_block.block.header.header.number);
            assert_eq!(block, dummy_block.block.unseal());
            assert_eq!(tx, dummy_tx);
            assert_eq!(tx_exec, TxExecInfo::default());
        }
    }
}
