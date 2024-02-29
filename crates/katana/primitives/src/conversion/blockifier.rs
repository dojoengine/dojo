//! Translation layer for converting the primitive types to the execution engine types.

use blockifier::execution::contract_class::{ContractClass, ContractClassV0, ContractClassV1};
use cairo_vm::types::errors::program_errors::ProgramError;
use starknet::core::utils::parse_cairo_short_string;
use starknet_api::core::{ContractAddress, PatriciaKey};
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;

use crate::chain::ChainId;
use crate::contract::CompiledClass;

impl From<crate::contract::ContractAddress> for ContractAddress {
    fn from(address: crate::contract::ContractAddress) -> Self {
        Self(patricia_key!(address.0))
    }
}

impl From<ContractAddress> for crate::contract::ContractAddress {
    fn from(address: ContractAddress) -> Self {
        Self((*address.0.key()).into())
    }
}

impl From<ChainId> for starknet_api::core::ChainId {
    fn from(chain_id: ChainId) -> Self {
        let name: String = match chain_id {
            ChainId::Named(named) => named.name().to_string(),
            ChainId::Id(id) => parse_cairo_short_string(&id).expect("valid cairo string"),
        };
        Self(name)
    }
}

pub fn to_class(class: CompiledClass) -> Result<ContractClass, ProgramError> {
    match class {
        CompiledClass::Deprecated(class) => {
            Ok(ContractClass::V0(ContractClassV0::try_from(class)?))
        }

        CompiledClass::Class(class) => {
            Ok(ContractClass::V1(ContractClassV1::try_from(class.casm)?))
        }
    }
}

#[cfg(test)]
mod tests {
    use starknet::core::utils::parse_cairo_short_string;

    use crate::chain::{ChainId, NamedChainId};

    #[test]
    fn convert_chain_id() {
        let mainnet = starknet_api::core::ChainId::from(ChainId::Named(NamedChainId::Mainnet));
        let goerli = starknet_api::core::ChainId::from(ChainId::Named(NamedChainId::Goerli));
        let sepolia = starknet_api::core::ChainId::from(ChainId::Named(NamedChainId::Sepolia));

        assert_eq!(mainnet.0, parse_cairo_short_string(&NamedChainId::Mainnet.id()).unwrap());
        assert_eq!(goerli.0, parse_cairo_short_string(&NamedChainId::Goerli.id()).unwrap());
        assert_eq!(sepolia.0, parse_cairo_short_string(&NamedChainId::Sepolia.id()).unwrap());
    }
}
