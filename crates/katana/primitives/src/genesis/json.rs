//! JSON representation of the genesis configuration. Used to deserialize the genesis configuration
//! from a JSON file.

use std::collections::{hash_map, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fs, io};

use cairo_lang_starknet::casm_contract_class::{CasmContractClass, StarknetSierraCompilationError};
use cairo_lang_starknet::contract_class::ContractClass;
use cairo_vm::types::errors::program_errors::ProgramError;
use ethers::types::U256;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::Deserialize;
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::contract::{ComputeClassHashError, JsonError};
use starknet::core::types::FromByteArrayError;

use super::allocation::{GenesisAccount, GenesisAccountAlloc, GenesisContractAlloc};
use super::constant::{
    DEFAULT_FEE_TOKEN_ADDRESS, DEFAULT_LEGACY_ERC20_CONTRACT_CASM,
    DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH, DEFAULT_LEGACY_ERC20_CONTRACT_COMPILED_CLASS_HASH,
    DEFAULT_LEGACY_UDC_CASM, DEFAULT_LEGACY_UDC_CLASS_HASH, DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH,
    DEFAULT_OZ_ACCOUNT_CONTRACT, DEFAULT_OZ_ACCOUNT_CONTRACT_CASM,
    DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH, DEFAULT_OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH,
    DEFAULT_UDC_ADDRESS,
};
use super::{FeeTokenConfig, Genesis, GenesisAllocation, UniversalDeployerConfig};
use crate::block::{BlockHash, BlockNumber, GasPrices};
use crate::contract::{
    ClassHash, CompiledContractClass, CompiledContractClassV1, ContractAddress, StorageKey,
    StorageValue,
};
use crate::genesis::GenesisClass;
use crate::utils::class::{parse_compiled_class_v0, parse_sierra_class};
use crate::FieldElement;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenesisClassJson {
    pub path: PathBuf,
    /// The class hash of the contract. If not provided, the class hash is computed from the
    /// class at `path`.
    pub class_hash: Option<ClassHash>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeTokenConfigJson {
    pub name: String,
    pub symbol: String,
    pub address: Option<ContractAddress>,
    pub decimals: u8,
    /// The class hash of the fee token contract.
    /// If not provided, the default fee token class is used.
    pub class: Option<ClassHash>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UniversalDeployerConfigJson {
    /// The address of the universal deployer contract.
    /// If not provided, the default UD address is used.
    pub address: Option<ContractAddress>,
    /// The class hash of the universal deployer contract.
    /// If not provided, the default UD class is used.
    pub class: Option<ClassHash>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenesisContractJson {
    pub class: ClassHash,
    pub balance: Option<U256>,
    pub nonce: Option<FieldElement>,
    pub storage: Option<HashMap<StorageKey, StorageValue>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenesisAccountJson {
    /// The public key of the account.
    pub public_key: FieldElement,
    pub balance: Option<U256>,
    pub nonce: Option<FieldElement>,
    /// The class hash of the account contract. If not provided, the default account class is used.
    pub class: Option<ClassHash>,
    pub storage: Option<HashMap<StorageKey, StorageValue>>,
}

/// A wrapper around [GenesisJson] that also contains the path to the JSON file. The `base_path` is
/// needed to calculate the paths of the class files, which are relative to the JSON file.
#[derive(Debug, Clone)]
pub struct GenesisJsonWithBasePath {
    pub base_path: PathBuf,
    pub content: GenesisJson,
}

impl GenesisJsonWithBasePath {
    /// Loads the genesis configuration from a JSON file at `path`.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, io::Error> {
        let content = fs::read_to_string(path.as_ref())?;
        let content: GenesisJson = serde_json::from_str(&content)?;

        let mut base_path = path.as_ref().to_path_buf();
        base_path.pop();

        Ok(Self { content, base_path })
    }

    /// Creates a new instance of [GenesisJsonWithBasePath] with the given `base_path` and
    /// `content`. If `base_path` is a path to a file, the parent directory is used as the base
    /// path.
    pub fn new_with_content_and_base_path(base_path: PathBuf, content: GenesisJson) -> Self {
        let base_path = if !base_path.is_dir() {
            let mut base_path = base_path;
            base_path.pop();
            base_path
        } else {
            base_path
        };
        Self { content, base_path }
    }
}

/// The JSON representation of the [Genesis] configuration. This `struct` is used to deserialize
/// the genesis configuration from a JSON file before being converted to a [Genesis] instance.
/// However, this type alone is inadequate for creating the [Genesis] type, for that you have to
/// load the JSON file using [GenesisJsonWithBasePath] and then convert it to [Genesis] using
/// [`Genesis::try_from<Genesis>`]. This is because the `classes` field of this type contains
/// paths to the class files, which are set to be relative to the JSON file.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenesisJson {
    pub parent_hash: BlockHash,
    pub state_root: FieldElement,
    pub number: BlockNumber,
    pub timestamp: u64,
    pub sequencer_address: ContractAddress,
    pub gas_prices: GasPrices,
    #[serde(default)]
    pub classes: Vec<GenesisClassJson>,
    pub fee_token: FeeTokenConfigJson,
    pub universal_deployer: Option<UniversalDeployerConfigJson>,
    #[serde(default)]
    pub accounts: HashMap<ContractAddress, GenesisAccountJson>,
    #[serde(default)]
    pub contracts: HashMap<ContractAddress, GenesisContractJson>,
}

#[derive(Debug, thiserror::Error)]
pub enum GenesisTryFromJsonError {
    #[error("Failed to read class file at path {path}: {source}")]
    FileNotFound { source: io::Error, path: PathBuf },
    #[error(transparent)]
    ParsingError(#[from] serde_json::Error),
    #[error(transparent)]
    ComputeClassHash(#[from] ComputeClassHashError),
    #[error(transparent)]
    ConversionError(#[from] FromByteArrayError),
    #[error(transparent)]
    SierraCompilation(#[from] StarknetSierraCompilationError),
    #[error(transparent)]
    ProgramError(#[from] ProgramError),
    #[error("Missing class entry for class hash {0}")]
    MissingClass(ClassHash),
    #[error("Failed to flatten Sierra contract: {0}")]
    FlattenSierraClass(#[from] JsonError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl TryFrom<GenesisJsonWithBasePath> for Genesis {
    type Error = GenesisTryFromJsonError;

    fn try_from(value: GenesisJsonWithBasePath) -> Result<Self, Self::Error> {
        let GenesisJsonWithBasePath { content: value, base_path } = value;

        let mut classes: HashMap<ClassHash, GenesisClass> = value
            .classes
            .into_par_iter()
            .map(|entry| {
                let mut path = base_path.clone();
                path.push(&entry.path);

                let path = path
                    .canonicalize()
                    .map_err(|e| GenesisTryFromJsonError::FileNotFound { source: e, path })?;

                // read the file at the path
                let content = fs::read_to_string(&path)
                    .map_err(|e| GenesisTryFromJsonError::FileNotFound { source: e, path })?;

                let (class_hash, compiled_class_hash, sierra, casm) =
                    match parse_sierra_class(&content) {
                        Ok(sierra) => {
                            let casm: ContractClass = serde_json::from_str(&content)?;
                            let casm = CasmContractClass::from_contract_class(casm, true)?;

                            // check if the class hash is provided, otherwise compute it form the
                            // artifacts
                            let class_hash = entry.class_hash.unwrap_or(sierra.class_hash()?);
                            let compiled_hash = casm.compiled_class_hash().to_be_bytes();

                            (
                                class_hash,
                                FieldElement::from_bytes_be(&compiled_hash)?,
                                Some(Arc::new(sierra.flatten()?)),
                                Arc::new(CompiledContractClass::V1(
                                    CompiledContractClassV1::try_from(casm)?,
                                )),
                            )
                        }

                        Err(_) => {
                            let casm = parse_compiled_class_v0(&content)?;

                            let class_hash = if let Some(class_hash) = entry.class_hash {
                                class_hash
                            } else {
                                let casm = serde_json::from_str::<LegacyContractClass>(&content)?;
                                casm.class_hash()?
                            };

                            (
                                class_hash,
                                class_hash,
                                None,
                                Arc::new(CompiledContractClass::V0(casm)),
                            )
                        }
                    };

                Ok((class_hash, GenesisClass { compiled_class_hash, sierra, casm }))
            })
            .collect::<Result<_, Self::Error>>()?;

        let mut fee_token = FeeTokenConfig {
            name: value.fee_token.name,
            symbol: value.fee_token.symbol,
            total_supply: U256::zero(),
            decimals: value.fee_token.decimals,
            address: value.fee_token.address.unwrap_or(DEFAULT_FEE_TOKEN_ADDRESS),
            class_hash: value.fee_token.class.unwrap_or(DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH),
        };

        match value.fee_token.class {
            Some(hash) => {
                if !classes.contains_key(&hash) {
                    return Err(GenesisTryFromJsonError::MissingClass(hash));
                }
            }

            // if no class hash is provided, use the default fee token class
            None => {
                let _ = classes.insert(
                    DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH,
                    GenesisClass {
                        sierra: None,
                        casm: Arc::new(DEFAULT_LEGACY_ERC20_CONTRACT_CASM.clone()),
                        compiled_class_hash: DEFAULT_LEGACY_ERC20_CONTRACT_COMPILED_CLASS_HASH,
                    },
                );
            }
        };

        let universal_deployer = if let Some(config) = value.universal_deployer {
            match config.class {
                Some(hash) => {
                    if !classes.contains_key(&hash) {
                        return Err(GenesisTryFromJsonError::MissingClass(hash));
                    }

                    Some(UniversalDeployerConfig {
                        class_hash: hash,
                        address: config.address.unwrap_or(DEFAULT_UDC_ADDRESS),
                    })
                }

                // if no class hash is provided, use the default UD contract parameters
                None => {
                    let class_hash = DEFAULT_LEGACY_UDC_CLASS_HASH;
                    let address = config.address.unwrap_or(DEFAULT_UDC_ADDRESS);

                    let _ = classes.insert(
                        DEFAULT_LEGACY_UDC_CLASS_HASH,
                        GenesisClass {
                            sierra: None,
                            casm: Arc::new(DEFAULT_LEGACY_UDC_CASM.clone()),
                            compiled_class_hash: DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH,
                        },
                    );

                    Some(UniversalDeployerConfig { class_hash, address })
                }
            }
        } else {
            None
        };

        let mut allocations: HashMap<ContractAddress, GenesisAllocation> = HashMap::new();

        for (address, account) in value.accounts {
            // check that the class hash exists in the classes field
            let class_hash = match account.class {
                Some(hash) => {
                    if !classes.contains_key(&hash) {
                        return Err(GenesisTryFromJsonError::MissingClass(hash));
                    } else {
                        hash
                    }
                }

                None => {
                    // check that the default account class exists in the classes field before
                    // inserting it
                    if let hash_map::Entry::Vacant(e) =
                        classes.entry(DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH)
                    {
                        // insert default account class to the classes map
                        e.insert(GenesisClass {
                            casm: Arc::new(DEFAULT_OZ_ACCOUNT_CONTRACT_CASM.clone()),
                            sierra: Some(Arc::new(
                                DEFAULT_OZ_ACCOUNT_CONTRACT.clone().flatten().unwrap(),
                            )),
                            compiled_class_hash: DEFAULT_OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH,
                        });
                    }

                    DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH
                }
            };

            let balance = account.balance.unwrap_or_default();
            // increase the total supply of the fee token
            fee_token.total_supply += balance;

            allocations.insert(
                address,
                GenesisAllocation::Account(GenesisAccountAlloc::Account(GenesisAccount {
                    balance,
                    class_hash,
                    nonce: account.nonce,
                    storage: account.storage,
                    public_key: account.public_key,
                })),
            );
        }

        for (address, contract) in value.contracts {
            // check that the class hash exists in the classes field
            let class_hash = contract.class;
            if !classes.contains_key(&contract.class) {
                return Err(GenesisTryFromJsonError::MissingClass(class_hash));
            }

            let balance = contract.balance.unwrap_or_default();
            // increase the total supply of the fee token
            fee_token.total_supply += balance;

            allocations.insert(
                address,
                GenesisAllocation::Contract(GenesisContractAlloc {
                    balance,
                    class_hash,
                    nonce: contract.nonce,
                    storage: contract.storage,
                }),
            );
        }

        Ok(Self {
            classes,
            fee_token,
            allocations,
            universal_deployer,
            number: value.number,
            sequencer_address: value.sequencer_address,
            timestamp: value.timestamp,
            gas_prices: value.gas_prices,
            state_root: value.state_root,
            parent_hash: value.parent_hash,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::str::FromStr;

    use ethers::types::U256;
    use starknet::macros::felt;

    use super::{GenesisClassJson, GenesisJson};
    use crate::block::GasPrices;
    use crate::genesis::allocation::{GenesisAccount, GenesisAccountAlloc, GenesisContractAlloc};
    use crate::genesis::constant::{
        DEFAULT_FEE_TOKEN_ADDRESS, DEFAULT_LEGACY_ERC20_CONTRACT_CASM,
        DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH,
        DEFAULT_LEGACY_ERC20_CONTRACT_COMPILED_CLASS_HASH, DEFAULT_LEGACY_UDC_CASM,
        DEFAULT_LEGACY_UDC_CLASS_HASH, DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH,
        DEFAULT_OZ_ACCOUNT_CONTRACT, DEFAULT_OZ_ACCOUNT_CONTRACT_CASM,
        DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH, DEFAULT_OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH,
        DEFAULT_UDC_ADDRESS,
    };
    use crate::genesis::json::GenesisJsonWithBasePath;
    use crate::genesis::{
        ContractAddress, FeeTokenConfig, Genesis, GenesisAllocation, GenesisClass,
        UniversalDeployerConfig,
    };

    fn genesis_json() -> GenesisJsonWithBasePath {
        GenesisJsonWithBasePath::new("./src/genesis/test-genesis.json").unwrap()
    }

    #[test]
    fn deserialize_from_json() {
        let genesis: GenesisJson = genesis_json().content;

        assert_eq!(genesis.number, 0);
        assert_eq!(genesis.parent_hash, felt!("0x999"));
        assert_eq!(genesis.timestamp, 5123512314u64);
        assert_eq!(genesis.state_root, felt!("0x99"));
        assert_eq!(genesis.gas_prices.eth, 1111);
        assert_eq!(genesis.gas_prices.strk, 2222);

        assert_eq!(genesis.fee_token.address, Some(ContractAddress::from(felt!("0x55"))));
        assert_eq!(genesis.fee_token.name, String::from("ETHER"));
        assert_eq!(genesis.fee_token.symbol, String::from("ETH"));
        assert_eq!(genesis.fee_token.class, Some(felt!("0x8")));
        assert_eq!(genesis.fee_token.decimals, 18);

        assert_eq!(
            genesis.universal_deployer.clone().unwrap().address,
            Some(ContractAddress::from(felt!("0x77")))
        );
        assert_eq!(genesis.universal_deployer.unwrap().class, None);

        let acc_1 = ContractAddress::from(felt!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"));
        let acc_2 = ContractAddress::from(felt!("0x70997970C51812dc3A010C7d01b50e0d17dc79C8"));

        assert_eq!(genesis.accounts.len(), 2);
        assert_eq!(genesis.accounts[&acc_1].public_key, felt!("0x1"));
        assert_eq!(
            genesis.accounts[&acc_1].balance,
            Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap())
        );
        assert_eq!(genesis.accounts[&acc_1].nonce, Some(felt!("0x1")));
        assert_eq!(genesis.accounts[&acc_1].class, Some(felt!("0x80085")));
        assert_eq!(
            genesis.accounts[&acc_1].storage,
            Some(HashMap::from([(felt!("0x1"), felt!("0x1")), (felt!("0x2"), felt!("0x2")),]))
        );

        assert_eq!(genesis.accounts[&acc_2].public_key, felt!("0x2"));
        assert_eq!(
            genesis.accounts[&acc_2].balance,
            Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap())
        );

        assert_eq!(genesis.contracts.len(), 2);

        assert_eq!(
            genesis.contracts[&ContractAddress::from(felt!("0xbaba"))].balance,
            Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap())
        );
        assert_eq!(genesis.contracts[&ContractAddress::from(felt!("0xbaba"))].nonce, None);
        assert_eq!(genesis.contracts[&ContractAddress::from(felt!("0xbaba"))].class, felt!("0x8"));

        assert_eq!(
            genesis.contracts[&ContractAddress::from(felt!("0xbab1"))].balance,
            Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap())
        );
        assert_eq!(genesis.contracts[&ContractAddress::from(felt!("0xbab1"))].nonce, None);
        assert_eq!(genesis.contracts[&ContractAddress::from(felt!("0xbab1"))].class, felt!("0x8"));

        assert_eq!(
            genesis.classes,
            vec![
                GenesisClassJson {
                    class_hash: Some(felt!("0x8")),
                    path: PathBuf::from("../../contracts/compiled/erc20.json"),
                },
                GenesisClassJson {
                    class_hash: Some(felt!("0x80085")),
                    path: PathBuf::from("../../contracts/compiled/universal_deployer.json"),
                },
                GenesisClassJson {
                    class_hash: Some(felt!("0xa55")),
                    path: PathBuf::from("../../contracts/compiled/oz_account_080.json"),
                },
            ]
        );
    }

    #[test]
    fn genesis_try_from_json() {
        let genesis = genesis_json();
        let actual_genesis = Genesis::try_from(genesis).unwrap();

        let classes = HashMap::from([
            (
                felt!("0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69"),
                GenesisClass {
                    sierra: None,
                    casm: DEFAULT_LEGACY_UDC_CASM.clone().into(),
                    compiled_class_hash: felt!(
                        "0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69"
                    ),
                },
            ),
            (
                felt!("0x80085"),
                GenesisClass {
                    sierra: None,
                    casm: DEFAULT_LEGACY_UDC_CASM.clone().into(),
                    compiled_class_hash: felt!("0x80085"),
                },
            ),
            (
                felt!("0x8"),
                GenesisClass {
                    sierra: None,
                    compiled_class_hash: felt!("0x8"),
                    casm: DEFAULT_LEGACY_ERC20_CONTRACT_CASM.clone().into(),
                },
            ),
            (
                DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
                GenesisClass {
                    compiled_class_hash: DEFAULT_OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH,
                    casm: DEFAULT_OZ_ACCOUNT_CONTRACT_CASM.clone().into(),
                    sierra: Some(DEFAULT_OZ_ACCOUNT_CONTRACT.clone().flatten().unwrap().into()),
                },
            ),
            (
                felt!("0xa55"),
                GenesisClass {
                    compiled_class_hash: DEFAULT_OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH,
                    casm: DEFAULT_OZ_ACCOUNT_CONTRACT_CASM.clone().into(),
                    sierra: Some(DEFAULT_OZ_ACCOUNT_CONTRACT.clone().flatten().unwrap().into()),
                },
            ),
        ]);

        let fee_token = FeeTokenConfig {
            address: ContractAddress::from(felt!("0x55")),
            name: String::from("ETHER"),
            symbol: String::from("ETH"),
            total_supply: U256::from_str("0xD3C21BCECCEDA1000000").unwrap() * 4,
            decimals: 18,
            class_hash: felt!("0x8"),
        };

        let allocations = HashMap::from([
            (
                ContractAddress::from(felt!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")),
                GenesisAllocation::Account(GenesisAccountAlloc::Account(GenesisAccount {
                    public_key: felt!("0x1"),
                    balance: U256::from_str("0xD3C21BCECCEDA1000000").unwrap(),
                    nonce: Some(felt!("0x1")),
                    class_hash: felt!("0x80085"),
                    storage: Some(HashMap::from([
                        (felt!("0x1"), felt!("0x1")),
                        (felt!("0x2"), felt!("0x2")),
                    ])),
                })),
            ),
            (
                ContractAddress::from(felt!("0x70997970C51812dc3A010C7d01b50e0d17dc79C8")),
                GenesisAllocation::Account(GenesisAccountAlloc::Account(GenesisAccount {
                    public_key: felt!("0x2"),
                    balance: U256::from_str("0xD3C21BCECCEDA1000000").unwrap(),
                    class_hash: DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
                    nonce: None,
                    storage: None,
                })),
            ),
            (
                ContractAddress::from(felt!("0xbaba")),
                GenesisAllocation::Contract(GenesisContractAlloc {
                    balance: U256::from_str("0xD3C21BCECCEDA1000000").unwrap(),
                    nonce: None,
                    class_hash: felt!("0x8"),
                    storage: Some(HashMap::from([
                        (felt!("0x1"), felt!("0x1")),
                        (felt!("0x2"), felt!("0x2")),
                    ])),
                }),
            ),
            (
                ContractAddress::from(felt!("0xbab1")),
                GenesisAllocation::Contract(GenesisContractAlloc {
                    balance: U256::from_str("0xD3C21BCECCEDA1000000").unwrap(),
                    nonce: None,
                    class_hash: felt!("0x8"),
                    storage: None,
                }),
            ),
        ]);

        let expected_genesis = Genesis {
            classes,
            number: 0,
            fee_token,
            allocations,
            timestamp: 5123512314u64,
            sequencer_address: ContractAddress::from(felt!("0x100")),
            state_root: felt!("0x99"),
            parent_hash: felt!("0x999"),
            gas_prices: GasPrices { eth: 1111, strk: 2222 },
            universal_deployer: Some(UniversalDeployerConfig {
                class_hash: DEFAULT_LEGACY_UDC_CLASS_HASH,
                address: ContractAddress::from(felt!("0x77")),
            }),
        };

        assert_eq!(actual_genesis.number, expected_genesis.number);
        assert_eq!(actual_genesis.parent_hash, expected_genesis.parent_hash);
        assert_eq!(actual_genesis.timestamp, expected_genesis.timestamp);
        assert_eq!(actual_genesis.state_root, expected_genesis.state_root);
        assert_eq!(actual_genesis.gas_prices, expected_genesis.gas_prices);

        assert_eq!(actual_genesis.fee_token.address, expected_genesis.fee_token.address);
        assert_eq!(actual_genesis.fee_token.name, expected_genesis.fee_token.name);
        assert_eq!(actual_genesis.fee_token.symbol, expected_genesis.fee_token.symbol);
        assert_eq!(actual_genesis.fee_token.decimals, expected_genesis.fee_token.decimals);
        assert_eq!(actual_genesis.fee_token.total_supply, expected_genesis.fee_token.total_supply);
        assert_eq!(actual_genesis.fee_token.class_hash, expected_genesis.fee_token.class_hash);

        assert_eq!(actual_genesis.universal_deployer, expected_genesis.universal_deployer);

        assert_eq!(actual_genesis.allocations.len(), expected_genesis.allocations.len());

        for alloc in actual_genesis.allocations {
            let expected_alloc = expected_genesis.allocations.get(&alloc.0).unwrap();
            assert_eq!(alloc.1, *expected_alloc);
        }

        assert_eq!(actual_genesis.classes.len(), expected_genesis.classes.len());

        for class in actual_genesis.classes {
            let expected_class = expected_genesis.classes.get(&class.0).unwrap();
            assert_eq!(class.1.compiled_class_hash, expected_class.compiled_class_hash);
            assert_eq!(class.1.casm, expected_class.casm);
            assert_eq!(class.1.sierra, expected_class.sierra.clone());
        }
    }

    #[test]
    fn default_genesis_try_from_json() {
        let json = r#"
        {
            "number": 0,
            "parentHash": "0x999",
            "timestamp": 5123512314,
            "stateRoot": "0x99",
            "sequencerAddress": "0x100",
            "gasPrices": {
                "ETH": 1111,
                "STRK": 2222
            },
            "feeToken": {
                "name": "ETHER",
                "symbol": "ETH",
                "decimals": 18
            },
            "universalDeployer": {},
            "accounts": {
                "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266": {
                    "publicKey": "0x1",
                    "balance": "0xD3C21BCECCEDA1000000"
                }
            },
            "contracts": {},
            "classes": []
        }
        "#;

        let content: GenesisJson = serde_json::from_str(json).unwrap();

        let base_path = PathBuf::from_str("../").unwrap().canonicalize().unwrap();
        let genesis_json =
            GenesisJsonWithBasePath::new_with_content_and_base_path(base_path, content);

        let actual_genesis = Genesis::try_from(genesis_json).unwrap();

        let classes = HashMap::from([
            (
                DEFAULT_LEGACY_UDC_CLASS_HASH,
                GenesisClass {
                    sierra: None,
                    casm: DEFAULT_LEGACY_UDC_CASM.clone().into(),
                    compiled_class_hash: DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH,
                },
            ),
            (
                DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH,
                GenesisClass {
                    sierra: None,
                    casm: DEFAULT_LEGACY_ERC20_CONTRACT_CASM.clone().into(),
                    compiled_class_hash: DEFAULT_LEGACY_ERC20_CONTRACT_COMPILED_CLASS_HASH,
                },
            ),
            (
                DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
                GenesisClass {
                    compiled_class_hash: DEFAULT_OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH,
                    casm: DEFAULT_OZ_ACCOUNT_CONTRACT_CASM.clone().into(),
                    sierra: Some(DEFAULT_OZ_ACCOUNT_CONTRACT.clone().flatten().unwrap().into()),
                },
            ),
        ]);

        let fee_token = FeeTokenConfig {
            address: DEFAULT_FEE_TOKEN_ADDRESS,
            name: String::from("ETHER"),
            symbol: String::from("ETH"),
            total_supply: U256::from_str("0xD3C21BCECCEDA1000000").unwrap(),
            decimals: 18,
            class_hash: DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH,
        };

        let allocations = HashMap::from([(
            ContractAddress::from(felt!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")),
            GenesisAllocation::Account(GenesisAccountAlloc::Account(GenesisAccount {
                public_key: felt!("0x1"),
                balance: U256::from_str("0xD3C21BCECCEDA1000000").unwrap(),
                class_hash: DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
                nonce: None,
                storage: None,
            })),
        )]);

        let expected_genesis = Genesis {
            fee_token,
            classes,
            allocations,
            number: 0,
            timestamp: 5123512314u64,
            state_root: felt!("0x99"),
            parent_hash: felt!("0x999"),
            sequencer_address: ContractAddress(felt!("0x100")),
            gas_prices: GasPrices { eth: 1111, strk: 2222 },
            universal_deployer: Some(UniversalDeployerConfig {
                class_hash: DEFAULT_LEGACY_UDC_CLASS_HASH,
                address: DEFAULT_UDC_ADDRESS,
            }),
        };

        assert_eq!(actual_genesis.universal_deployer, expected_genesis.universal_deployer);
        assert_eq!(actual_genesis.allocations.len(), expected_genesis.allocations.len());

        for (address, alloc) in actual_genesis.allocations {
            let expected_alloc = expected_genesis.allocations.get(&address).unwrap();
            assert_eq!(alloc, *expected_alloc);
        }

        // assert that the list of classes is the same
        assert_eq!(actual_genesis.classes.len(), expected_genesis.classes.len());

        for (hash, class) in actual_genesis.classes {
            let expected_class = expected_genesis.classes.get(&hash).unwrap();

            assert_eq!(class.compiled_class_hash, expected_class.compiled_class_hash);
            assert_eq!(class.casm, expected_class.casm);
            assert_eq!(class.sierra, expected_class.sierra.clone());
        }
    }
}
