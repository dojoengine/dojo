use katana_primitives::block::{ExecutableBlock, PartialHeader};
use katana_primitives::chain::ChainId;
use katana_primitives::contract::ContractAddress;
use katana_primitives::da::L1DataAvailabilityMode;
use katana_primitives::genesis::Genesis;
use katana_primitives::version::CURRENT_STARKNET_VERSION;
use serde::{Deserialize, Serialize};

pub mod file;
mod utils;

use crate::SettlementLayer;

/// The rollup chain specification.
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ChainSpec {
    /// The rollup network chain id.
    pub id: ChainId,

    /// The chain's genesis states.
    pub genesis: Genesis,

    /// The chain fee token contract.
    pub fee_contract: FeeContract,

    /// The chain's settlement layer configurations.
    pub settlement: SettlementLayer,
}

//////////////////////////////////////////////////////////////
// 	ChainSpec implementations
//////////////////////////////////////////////////////////////

impl ChainSpec {
    pub fn block(&self) -> ExecutableBlock {
        let header = PartialHeader {
            protocol_version: CURRENT_STARKNET_VERSION,
            number: self.genesis.number,
            timestamp: self.genesis.timestamp,
            parent_hash: self.genesis.parent_hash,
            l1_da_mode: L1DataAvailabilityMode::Calldata,
            l1_gas_prices: self.genesis.gas_prices.clone(),
            l1_data_gas_prices: self.genesis.gas_prices.clone(),
            sequencer_address: self.genesis.sequencer_address,
        };

        let transactions = utils::GenesisTransactionsBuilder::new(self).build();

        ExecutableBlock { header, body: transactions }
    }
}

/// Token that can be used for transaction fee payments on the chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct FeeContract {
    pub strk: ContractAddress,
}
