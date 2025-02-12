use katana_primitives::block::BlockNumber;
use katana_primitives::chain::ChainId;
use katana_primitives::genesis::Genesis;
use katana_primitives::{eth, ContractAddress};
use serde::{Deserialize, Serialize};
use url::Url;

pub mod dev;
pub mod rollup;

#[derive(Debug, Clone)]
pub enum ChainSpec {
    Dev(dev::ChainSpec),
    Rollup(rollup::ChainSpec),
}

//////////////////////////////////////////////////////////////
// 	ChainSpec implementations
//////////////////////////////////////////////////////////////

impl ChainSpec {
    /// Creates a new [`ChainSpec`] for a development chain.
    pub fn dev() -> Self {
        Self::Dev(dev::DEV.clone())
    }

    pub fn id(&self) -> ChainId {
        match self {
            Self::Dev(spec) => spec.id,
            Self::Rollup(spec) => spec.id,
        }
    }

    pub fn genesis(&self) -> &Genesis {
        match self {
            Self::Dev(spec) => &spec.genesis,
            Self::Rollup(spec) => &spec.genesis,
        }
    }

    pub fn settlement(&self) -> Option<&SettlementLayer> {
        match self {
            Self::Dev(spec) => spec.settlement.as_ref(),
            Self::Rollup(spec) => Some(&spec.settlement),
        }
    }
}

impl From<dev::ChainSpec> for ChainSpec {
    fn from(spec: dev::ChainSpec) -> Self {
        Self::Dev(spec)
    }
}

impl From<rollup::ChainSpec> for ChainSpec {
    fn from(spec: rollup::ChainSpec) -> Self {
        Self::Rollup(spec)
    }
}

impl Default for ChainSpec {
    fn default() -> Self {
        Self::dev()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "kebab-case")]
pub enum SettlementLayer {
    Ethereum {
        // The id of the settlement chain.
        id: eth::ChainId,

        // url for ethereum rpc provider
        rpc_url: Url,

        /// account on the ethereum network
        account: eth::Address,

        // - The core appchain contract used to settlement
        core_contract: eth::Address,

        // the block at which the core contract was deployed
        block: alloy_primitives::BlockNumber,
    },

    Starknet {
        // The id of the settlement chain.
        id: ChainId,

        // url for starknet rpc provider
        rpc_url: Url,

        /// account on the starknet network
        account: ContractAddress,

        // - The core appchain contract used to settlement
        core_contract: ContractAddress,

        // the block at which the core contract was deployed
        block: BlockNumber,
    },
}
