use std::collections::HashMap;
use std::path::PathBuf;
use std::{fs, io};

use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_lang_starknet::contract_class::ContractClass;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::contract::{ComputeClassHashError, JsonError};

use super::Genesis;
use crate::block::{BlockHash, BlockNumber, GasPrices};
use crate::contract::{
    ClassHash, CompiledContractClass, CompiledContractClassV1, ContractAddress, StorageKey,
    StorageValue,
};
use crate::genesis::GenesisClass;
use crate::utils::class::{parse_compiled_class_v0, parse_sierra_class};
use crate::FieldElement;

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ClassHashOrPath {
    ClassHash(ClassHash),
    Path(PathBuf),
}

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GenesisClassJson {
    pub path: PathBuf,
    pub class_hash: Option<ClassHash>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeTokenConfigJson {
    pub name: String,
    pub symbol: String,
    pub address: ContractAddress,
    pub decimals: u8,
    pub class: Option<ClassHash>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct UniversalDeployerConfigJson {
    pub address: Option<ContractAddress>,
    pub class: Option<ClassHash>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenesisAccountJson {
    pub private_key: FieldElement,
    pub balance: FieldElement,
    pub nonce: Option<FieldElement>,
    pub class: Option<ClassHash>,
    pub storage: Option<HashMap<StorageKey, StorageValue>>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenesisJson {
    pub parent_hash: BlockHash,
    pub state_root: FieldElement,
    pub number: BlockNumber,
    pub timestamp: u128,
    pub gas_prices: GasPrices,
    #[serde(default)]
    pub classes: Vec<GenesisClassJson>,
    pub fee_token: FeeTokenConfigJson,
    pub universal_deployer: UniversalDeployerConfigJson,
    pub allocations: HashMap<ContractAddress, GenesisAccountJson>,
}

#[derive(Debug, thiserror::Error)]
pub enum GenesisTryFromJsonError {
    #[error("Failed to read contract class file: {0}")]
    FileNotFound(#[from] io::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
    #[error(transparent)]
    ComputeClassHash(#[from] ComputeClassHashError),
    #[error(transparent)]
    FlattenedError(#[from] JsonError),
}

impl TryFrom<GenesisJson> for Genesis {
    type Error = anyhow::Error;

    fn try_from(value: GenesisJson) -> Result<Self, Self::Error> {
        let mut classes: HashMap<ClassHash, GenesisClass> = value
            .classes
            .into_par_iter()
            .map(|class| {
                // read the file at the path
                let content = fs::read_to_string(class.path)?;
                let (class_hash, compiled_class_hash, sierra, casm) =
                    match parse_sierra_class(&content) {
                        Ok(sierra) => {
                            let casm: ContractClass = serde_json::from_str(&content)?;
                            let casm = CasmContractClass::from_contract_class(casm, true)?;

                            let class_hash = sierra.class_hash()?;
                            let compiled_hash = casm.compiled_class_hash().to_be_bytes();

                            (
                                class_hash,
                                FieldElement::from_bytes_be(&compiled_hash)?,
                                Some(sierra.flatten()?),
                                CompiledContractClass::V1(CompiledContractClassV1::try_from(casm)?),
                            )
                        }

                        Err(_) => {
                            let casm = parse_compiled_class_v0(&content)?;

                            let class_hash = {
                                let casm = serde_json::from_str::<LegacyContractClass>(&content)?;
                                casm.class_hash()?
                            };

                            (class_hash, class_hash, None, CompiledContractClass::V0(casm))
                        }
                    };

                Ok((class_hash, GenesisClass { compiled_class_hash, sierra, casm }))
            })
            .collect::<Result<_, Self::Error>>()?;

        // check fee token class whether its a path or a class hash
        // if class hash then make sure the class hash exist in `classes` otherwise
        // if path then read the file and parse it
        match value.fee_token.class {
            Some(hash) => {
                if !classes.contains_key(&hash) {
                    return Err(anyhow::anyhow!("Fee token class hash does not exist in classes"));
                }
            }

            /// if no class hash is provided, use the default fee token class
            None => {}
        };

        todo!()
        // Ok(Self {
        //     parent_hash: value.parent_hash,
        //     state_root: value.state_root,
        //     number: value.number,
        //     timestamp: value.timestamp,
        //     gas_prices: value.gas_prices,
        //     allocations: Default::default(),
        //     classes: Default::default(),
        // })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use serde_json::json;
    use starknet::macros::felt;

    use super::{ClassHashOrPath, GenesisClassJson, GenesisJson};
    use crate::contract::ContractAddress;

    #[test]
    fn deserialize_from_json() {
        let json = json!(
            {
                "number": 0,
                "parentHash": "0x999",
                "timestamp": 5123512314u128,
                "stateRoot": "0x99",
                "gasPrices": {
                    "ETH": 1241231,
                    "STRK": 123123
                },
                "feeToken": {
                    "address": "0x55",
                    "name": "ETHER",
                    "symbol": "ETH",
                    "class": "../../classes/erc20.json"
                },
                "universalDeployer": {
                    "address": "0x77",
                    "class": "../../classes/universal_deployer.json"
                },
                "allocations": {
                    "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266": {
                        "privateKey": "0x1",
                        "balance": "0xD3C21BCECCEDA1000000",
                        "nonce": "0x1",
                        "class": "0x80085",
                        "storage": {
                            "0x1": "0x1",
                            "0x2": "0x2"
                        }
                    },
                    "0x70997970C51812dc3A010C7d01b50e0d17dc79C8": {
                        "privateKey": "0x2",
                        "balance": "0xD3C21BCECCEDA1000000"
                    }
                },
                "classes": [
                    {
                        "path": "../../classes/contract.json"
                    },
                    {
                        "path": "../../classes/account.json",
                        "classHash": "0x80085"
                    }
                ]
            }
        );

        let genesis: GenesisJson = serde_json::from_value(json).unwrap();

        assert_eq!(genesis.number, 0);
        assert_eq!(genesis.parent_hash, felt!("0x999"));
        assert_eq!(genesis.timestamp, 5123512314u128);
        assert_eq!(genesis.state_root, felt!("0x99"));
        assert_eq!(genesis.gas_prices.eth, 1241231);
        assert_eq!(genesis.gas_prices.strk, 123123);

        assert_eq!(genesis.fee_token.address, ContractAddress::from(felt!("0x55")));
        assert_eq!(genesis.fee_token.name, String::from("ETHER"));
        assert_eq!(genesis.fee_token.symbol, String::from("ETH"));
        assert_eq!(
            genesis.fee_token.class.unwrap(),
            ClassHashOrPath::Path(PathBuf::from("../../classes/erc20.json"))
        );

        assert_eq!(genesis.universal_deployer.address, Some(ContractAddress::from(felt!("0x77"))));
        assert_eq!(
            genesis.universal_deployer.class,
            Some(ClassHashOrPath::Path(PathBuf::from("../../classes/universal_deployer.json")))
        );

        let alloc_1 = ContractAddress::from(felt!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"));
        let alloc_2 = ContractAddress::from(felt!("0x70997970C51812dc3A010C7d01b50e0d17dc79C8"));

        assert_eq!(genesis.allocations.len(), 2);
        assert_eq!(genesis.allocations[&alloc_1].private_key, felt!("0x1"));
        assert_eq!(genesis.allocations[&alloc_1].balance, felt!("0xD3C21BCECCEDA1000000"));
        assert_eq!(genesis.allocations[&alloc_1].nonce, Some(felt!("0x1")));
        assert_eq!(
            genesis.allocations[&alloc_1].class,
            Some(ClassHashOrPath::ClassHash(felt!("0x80085")))
        );
        assert_eq!(
            genesis.allocations[&alloc_1].storage,
            Some(HashMap::from([(felt!("0x1"), felt!("0x1")), (felt!("0x2"), felt!("0x2")),]))
        );

        assert_eq!(genesis.allocations[&alloc_2].private_key, felt!("0x2"));
        assert_eq!(genesis.allocations[&alloc_2].balance, felt!("0xD3C21BCECCEDA1000000"));

        assert_eq!(
            genesis.classes,
            vec![
                GenesisClassJson {
                    class_hash: None,
                    path: PathBuf::from("../../classes/contract.json"),
                },
                GenesisClassJson {
                    class_hash: Some(felt!("0x80085")),
                    path: PathBuf::from("../../classes/account.json"),
                },
            ]
        );
    }
}
