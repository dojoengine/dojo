use std::path::Path;

use anyhow::Result;
use katana_db::init_db;
use katana_db::utils::is_database_empty;
use katana_primitives::block::{
    Block, BlockHash, FinalityStatus, Header, PartialHeader, SealedBlockWithStatus,
};
use katana_primitives::env::BlockEnv;
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_primitives::version::CURRENT_STARKNET_VERSION;
use katana_primitives::FieldElement;
use katana_provider::providers::db::DbProvider;
use katana_provider::traits::block::{BlockProvider, BlockWriter};
use katana_provider::traits::contract::ContractClassWriter;
use katana_provider::traits::env::BlockEnvProvider;
use katana_provider::traits::state::{StateFactoryProvider, StateRootProvider, StateWriter};
use katana_provider::traits::state_update::StateUpdateProvider;
use katana_provider::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider, TransactionsProviderExt,
};
use katana_provider::BlockchainProvider;

use crate::constants::SEQUENCER_ADDRESS;
use crate::utils::get_genesis_states_for_testing;

pub trait Database:
    BlockProvider
    + BlockWriter
    + TransactionProvider
    + TransactionStatusProvider
    + TransactionsProviderExt
    + ReceiptProvider
    + StateUpdateProvider
    + StateRootProvider
    + StateWriter
    + ContractClassWriter
    + StateFactoryProvider
    + BlockEnvProvider
    + 'static
    + Send
    + Sync
{
}

impl<T> Database for T where
    T: BlockProvider
        + BlockWriter
        + TransactionProvider
        + TransactionStatusProvider
        + TransactionsProviderExt
        + ReceiptProvider
        + StateUpdateProvider
        + StateRootProvider
        + StateWriter
        + ContractClassWriter
        + StateFactoryProvider
        + BlockEnvProvider
        + 'static
        + Send
        + Sync
{
}

pub struct Blockchain {
    inner: BlockchainProvider<Box<dyn Database>>,
}

impl Blockchain {
    pub fn new(provider: impl Database) -> Self {
        Self { inner: BlockchainProvider::new(Box::new(provider)) }
    }

    pub fn new_with_genesis(provider: impl Database, block_env: &BlockEnv) -> Result<Self> {
        let header = PartialHeader {
            parent_hash: 0u8.into(),
            version: CURRENT_STARKNET_VERSION,
            timestamp: block_env.timestamp,
            sequencer_address: *SEQUENCER_ADDRESS,
            gas_prices: block_env.l1_gas_prices,
        };

        let block = SealedBlockWithStatus {
            status: FinalityStatus::AcceptedOnL1,
            block: Block {
                header: Header::new(header, block_env.number, 0u8.into()),
                body: vec![],
            }
            .seal(),
        };

        Self::new_with_block_and_state(provider, block, get_genesis_states_for_testing())
    }

    pub fn new_with_db(db_path: impl AsRef<Path>, block_context: &BlockEnv) -> Result<Self> {
        if is_database_empty(&db_path) {
            let provider = DbProvider::new(init_db(db_path)?);
            Ok(Self::new_with_genesis(provider, block_context)?)
        } else {
            Ok(Self::new(DbProvider::new(init_db(db_path)?)))
        }
    }

    // TODO: make this function to just accept a `Header` created from the forked block.
    /// Builds a new blockchain with a forked block.
    pub fn new_from_forked(
        provider: impl Database,
        block_hash: BlockHash,
        parent_hash: FieldElement,
        block_env: &BlockEnv,
        state_root: FieldElement,
        block_status: FinalityStatus,
    ) -> Result<Self> {
        let header = Header {
            state_root,
            parent_hash,
            version: CURRENT_STARKNET_VERSION,
            number: block_env.number,
            timestamp: block_env.timestamp,
            sequencer_address: *SEQUENCER_ADDRESS,
            gas_prices: block_env.l1_gas_prices,
        };

        let block = SealedBlockWithStatus {
            status: block_status,
            block: Block { header, body: vec![] }.seal_with_hash(block_hash),
        };

        Self::new_with_block_and_state(provider, block, StateUpdatesWithDeclaredClasses::default())
    }

    pub fn provider(&self) -> &BlockchainProvider<Box<dyn Database>> {
        &self.inner
    }

    fn new_with_block_and_state(
        provider: impl Database,
        block: SealedBlockWithStatus,
        states: StateUpdatesWithDeclaredClasses,
    ) -> Result<Self> {
        BlockWriter::insert_block_with_states_and_receipts(&provider, block, states, vec![])?;
        Ok(Self::new(provider))
    }
}

#[cfg(test)]
mod tests {
    use katana_primitives::block::{
        Block, FinalityStatus, GasPrices, Header, SealedBlockWithStatus,
    };
    use katana_primitives::env::BlockEnv;
    use katana_primitives::receipt::{InvokeTxReceipt, Receipt};
    use katana_primitives::state::StateUpdatesWithDeclaredClasses;
    use katana_primitives::transaction::{InvokeTx, Tx, TxWithHash};
    use katana_primitives::FieldElement;
    use katana_provider::providers::in_memory::InMemoryProvider;
    use katana_provider::traits::block::{
        BlockHashProvider, BlockNumberProvider, BlockProvider, BlockStatusProvider, BlockWriter,
        HeaderProvider,
    };
    use katana_provider::traits::state::StateFactoryProvider;
    use katana_provider::traits::transaction::TransactionProvider;
    use starknet::macros::felt;

    use super::Blockchain;
    use crate::constants::{
        ERC20_CONTRACT, ERC20_CONTRACT_CLASS_HASH, FEE_TOKEN_ADDRESS, UDC_ADDRESS, UDC_CLASS_HASH,
        UDC_CONTRACT,
    };

    #[test]
    fn blockchain_from_genesis_states() {
        let provider = InMemoryProvider::new();
        let block_env = BlockEnv {
            number: 0,
            timestamp: 0,
            sequencer_address: Default::default(),
            l1_gas_prices: GasPrices { eth: 0, strk: 0 },
        };

        let blockchain = Blockchain::new_with_genesis(provider, &block_env)
            .expect("failed to create blockchain from genesis block");
        let state = blockchain.provider().latest().expect("failed to get latest state");

        let latest_number = blockchain.provider().latest_number().unwrap();
        let fee_token_class_hash =
            state.class_hash_of_contract(*FEE_TOKEN_ADDRESS).unwrap().unwrap();
        let udc_class_hash = state.class_hash_of_contract(*UDC_ADDRESS).unwrap().unwrap();

        assert_eq!(latest_number, 0);
        assert_eq!(udc_class_hash, *UDC_CLASS_HASH);
        assert_eq!(fee_token_class_hash, *ERC20_CONTRACT_CLASS_HASH);
    }

    #[test]
    fn blockchain_from_fork() {
        let provider = InMemoryProvider::new();

        let block_env = BlockEnv {
            number: 23,
            timestamp: 6868,
            sequencer_address: Default::default(),
            l1_gas_prices: GasPrices { eth: 9090, strk: 0 },
        };

        let blockchain = Blockchain::new_from_forked(
            provider,
            felt!("1111"),
            FieldElement::ZERO,
            &block_env,
            felt!("1334"),
            FinalityStatus::AcceptedOnL1,
        )
        .expect("failed to create fork blockchain");

        let latest_number = blockchain.provider().latest_number().unwrap();
        let latest_hash = blockchain.provider().latest_hash().unwrap();
        let header = blockchain.provider().header(latest_number.into()).unwrap().unwrap();
        let block_status =
            blockchain.provider().block_status(latest_number.into()).unwrap().unwrap();

        assert_eq!(latest_number, 23);
        assert_eq!(latest_hash, felt!("1111"));

        assert_eq!(header.gas_prices.eth, 9090);
        assert_eq!(header.timestamp, 6868);
        assert_eq!(header.number, latest_number);
        assert_eq!(header.state_root, felt!("1334"));
        assert_eq!(header.parent_hash, FieldElement::ZERO);
        assert_eq!(block_status, FinalityStatus::AcceptedOnL1);
    }

    #[test]
    fn blockchain_from_db() {
        let db_path = tempfile::TempDir::new().expect("Failed to create temp dir.").into_path();

        let block_env = BlockEnv {
            number: 0,
            timestamp: 0,
            sequencer_address: Default::default(),
            l1_gas_prices: GasPrices { eth: 0, strk: 0 },
        };

        let dummy_tx =
            TxWithHash { hash: felt!("0xbad"), transaction: Tx::Invoke(InvokeTx::default()) };

        let dummy_block = SealedBlockWithStatus {
            status: FinalityStatus::AcceptedOnL1,
            block: Block {
                header: Header {
                    parent_hash: FieldElement::ZERO,
                    number: 1,
                    gas_prices: GasPrices::default(),
                    timestamp: 123456,
                    ..Default::default()
                },
                body: vec![dummy_tx.clone()],
            }
            .seal(),
        };

        {
            let blockchain = Blockchain::new_with_db(&db_path, &block_env)
                .expect("Failed to create db-backed blockchain storage");

            blockchain
                .provider()
                .insert_block_with_states_and_receipts(
                    dummy_block.clone(),
                    StateUpdatesWithDeclaredClasses::default(),
                    vec![Receipt::Invoke(InvokeTxReceipt::default())],
                )
                .unwrap();

            // assert genesis state is correct

            let state = blockchain.provider().latest().expect("failed to get latest state");

            let actual_udc_class_hash =
                state.class_hash_of_contract(*UDC_ADDRESS).unwrap().unwrap();
            let actual_udc_class = state.class(actual_udc_class_hash).unwrap().unwrap();

            let actual_fee_token_class_hash =
                state.class_hash_of_contract(*FEE_TOKEN_ADDRESS).unwrap().unwrap();
            let actual_fee_token_class = state.class(actual_fee_token_class_hash).unwrap().unwrap();

            assert_eq!(actual_udc_class_hash, *UDC_CLASS_HASH);
            assert_eq!(actual_udc_class, (*UDC_CONTRACT).clone());

            assert_eq!(actual_fee_token_class_hash, *ERC20_CONTRACT_CLASS_HASH);
            assert_eq!(actual_fee_token_class, (*ERC20_CONTRACT).clone());
        }

        // re open the db and assert the state is the same and not overwritten

        {
            let blockchain = Blockchain::new_with_db(&db_path, &block_env)
                .expect("Failed to create db-backed blockchain storage");

            // assert genesis state is correct

            let state = blockchain.provider().latest().expect("failed to get latest state");

            let actual_udc_class_hash =
                state.class_hash_of_contract(*UDC_ADDRESS).unwrap().unwrap();
            let actual_udc_class = state.class(actual_udc_class_hash).unwrap().unwrap();

            let actual_fee_token_class_hash =
                state.class_hash_of_contract(*FEE_TOKEN_ADDRESS).unwrap().unwrap();
            let actual_fee_token_class = state.class(actual_fee_token_class_hash).unwrap().unwrap();

            assert_eq!(actual_udc_class_hash, *UDC_CLASS_HASH);
            assert_eq!(actual_udc_class, (*UDC_CONTRACT).clone());

            assert_eq!(actual_fee_token_class_hash, *ERC20_CONTRACT_CLASS_HASH);
            assert_eq!(actual_fee_token_class, (*ERC20_CONTRACT).clone());

            let block_number = blockchain.provider().latest_number().unwrap();
            let block_hash = blockchain.provider().latest_hash().unwrap();
            let block = blockchain
                .provider()
                .block_by_hash(dummy_block.block.header.hash)
                .unwrap()
                .unwrap();

            let tx = blockchain.provider().transaction_by_hash(dummy_tx.hash).unwrap().unwrap();

            assert_eq!(block_hash, dummy_block.block.header.hash);
            assert_eq!(block_number, dummy_block.block.header.header.number);
            assert_eq!(block, dummy_block.block.unseal());
            assert_eq!(tx, dummy_tx);
        }
    }
}
