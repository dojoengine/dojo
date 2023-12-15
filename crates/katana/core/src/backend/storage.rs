use anyhow::Result;
use blockifier::block_context::BlockContext;
use katana_primitives::block::{
    Block, BlockHash, FinalityStatus, GasPrices, Header, PartialHeader, SealedBlockWithStatus,
};
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_primitives::version::CURRENT_STARKNET_VERSION;
use katana_primitives::FieldElement;
use katana_provider::traits::block::{BlockProvider, BlockWriter};
use katana_provider::traits::contract::ContractClassWriter;
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

    pub fn new_with_genesis(provider: impl Database, block_context: &BlockContext) -> Result<Self> {
        let header = PartialHeader {
            parent_hash: 0u8.into(),
            version: CURRENT_STARKNET_VERSION,
            timestamp: block_context.block_timestamp.0,
            sequencer_address: *SEQUENCER_ADDRESS,
            gas_prices: GasPrices {
                eth_gas_price: block_context.gas_prices.eth_l1_gas_price.try_into().unwrap(),
                strk_gas_price: block_context.gas_prices.strk_l1_gas_price.try_into().unwrap(),
            },
        };

        let block = SealedBlockWithStatus {
            status: FinalityStatus::AcceptedOnL1,
            block: Block {
                header: Header::new(header, block_context.block_number.0, 0u8.into()),
                body: vec![],
            }
            .seal(),
        };

        Self::new_with_block_and_state(provider, block, get_genesis_states_for_testing())
    }

    // TODO: make this function to just accept a `Header` created from the forked block.
    /// Builds a new blockchain with a forked block.
    pub fn new_from_forked(
        provider: impl Database,
        block_hash: BlockHash,
        parent_hash: FieldElement,
        block_context: &BlockContext,
        state_root: FieldElement,
        block_status: FinalityStatus,
    ) -> Result<Self> {
        let header = Header {
            state_root,
            parent_hash,
            version: CURRENT_STARKNET_VERSION,
            number: block_context.block_number.0,
            timestamp: block_context.block_timestamp.0,
            sequencer_address: *SEQUENCER_ADDRESS,
            gas_prices: GasPrices {
                eth_gas_price: block_context.gas_prices.eth_l1_gas_price.try_into().unwrap(),
                strk_gas_price: block_context.gas_prices.strk_l1_gas_price.try_into().unwrap(),
            },
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
    use blockifier::block_context::{BlockContext, FeeTokenAddresses, GasPrices};
    use katana_primitives::block::FinalityStatus;
    use katana_primitives::FieldElement;
    use katana_provider::providers::in_memory::InMemoryProvider;
    use katana_provider::traits::block::{
        BlockHashProvider, BlockNumberProvider, BlockStatusProvider, HeaderProvider,
    };
    use katana_provider::traits::state::StateFactoryProvider;
    use starknet::macros::felt;
    use starknet_api::block::{BlockNumber, BlockTimestamp};
    use starknet_api::core::ChainId;

    use super::Blockchain;
    use crate::constants::{
        ERC20_CONTRACT_CLASS_HASH, FEE_TOKEN_ADDRESS, UDC_ADDRESS, UDC_CLASS_HASH,
    };

    #[test]
    fn blockchain_from_genesis_states() {
        let provider = InMemoryProvider::new();
        let block_context = BlockContext {
            gas_prices: GasPrices { eth_l1_gas_price: 0, strk_l1_gas_price: 0 },
            max_recursion_depth: 0,
            validate_max_n_steps: 0,
            invoke_tx_max_n_steps: 0,
            block_number: BlockNumber(0),
            chain_id: ChainId("test".into()),
            block_timestamp: BlockTimestamp(0),
            sequencer_address: Default::default(),
            fee_token_addresses: FeeTokenAddresses {
                eth_fee_token_address: Default::default(),
                strk_fee_token_address: Default::default(),
            },
            vm_resource_fee_cost: Default::default(),
        };

        let blockchain = Blockchain::new_with_genesis(provider, &block_context)
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

        let block_context = BlockContext {
            gas_prices: GasPrices { eth_l1_gas_price: 9090, strk_l1_gas_price: 0 },
            max_recursion_depth: 0,
            validate_max_n_steps: 0,
            invoke_tx_max_n_steps: 0,
            chain_id: ChainId("test".into()),
            block_number: BlockNumber(23),
            block_timestamp: BlockTimestamp(6868),
            sequencer_address: Default::default(),
            fee_token_addresses: FeeTokenAddresses {
                eth_fee_token_address: Default::default(),
                strk_fee_token_address: Default::default(),
            },
            vm_resource_fee_cost: Default::default(),
        };

        let blockchain = Blockchain::new_from_forked(
            provider,
            felt!("1111"),
            FieldElement::ZERO,
            &block_context,
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

        assert_eq!(header.gas_prices.eth_gas_price, 9090);
        assert_eq!(header.timestamp, 6868);
        assert_eq!(header.number, latest_number);
        assert_eq!(header.state_root, felt!("1334"));
        assert_eq!(header.parent_hash, FieldElement::ZERO);
        assert_eq!(block_status, FinalityStatus::AcceptedOnL1);
    }
}
