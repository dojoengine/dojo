use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{fs, io};

use cairo_lang_starknet::casm_contract_class::{CasmContractClass, StarknetSierraCompilationError};
use cairo_lang_starknet::contract_class::ContractClass;
use cairo_vm::types::errors::program_errors::ProgramError;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::contract::{ComputeClassHashError, JsonError};
use starknet::core::types::FromByteArrayError;

use super::{
    FeeTokenConfig, Genesis, UniversalDeployerConfig, DEFAULT_LEGACY_ERC20_CONTRACT_CASM,
    DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH, DEFAULT_LEGACY_ERC20_CONTRACT_COMPILED_CLASS_HASH,
    DEFAULT_LEGACY_UDC_CASM, DEFAULT_LEGACY_UDC_CLASS_HASH, DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH,
    DEFAULT_OZ_ACCOUNT_CONTRACT, DEFAULT_OZ_ACCOUNT_CONTRACT_CASM,
    DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH, DEFAULT_OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH,
    DEFAULT_UDC_ADDRESS,
};
use crate::block::{BlockHash, BlockNumber, GasPrices};
use crate::contract::{
    ClassHash, CompiledContractClass, CompiledContractClassV1, ContractAddress, StorageKey,
    StorageValue,
};
use crate::genesis::{public_key_from_private_key, GenesisAccount, GenesisClass};
use crate::utils::class::{parse_compiled_class_v0, parse_sierra_class};
use crate::FieldElement;

#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(untagged)]
pub enum ClassHashOrPath {
    ClassHash(ClassHash),
    Path(PathBuf),
}

#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenesisClassJson {
    pub path: PathBuf,
    /// The class hash of the contract. If not provided, the class hash is computed from the
    /// class at `path`.
    pub class_hash: Option<ClassHash>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeTokenConfigJson {
    pub name: String,
    pub symbol: String,
    pub address: ContractAddress,
    pub decimals: u8,
    /// The class hash of the fee token contract.
    /// If not provided, the default fee token class is used.
    pub class: Option<ClassHash>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct UniversalDeployerConfigJson {
    /// The address of the universal deployer contract.
    /// If not provided, the default UD address is used.
    pub address: Option<ContractAddress>,
    /// The class hash of the universal deployer contract.
    /// If not provided, the default UD class is used.
    pub class: Option<ClassHash>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenesisAccountJson {
    pub private_key: FieldElement,
    pub balance: FieldElement,
    pub nonce: Option<FieldElement>,
    /// The class hash of the account contract. If not provided, the default account class is used.
    pub class: Option<ClassHash>,
    pub storage: Option<HashMap<StorageKey, StorageValue>>,
}

#[derive(Debug)]
pub struct GenesisJsonWithPath {
    base_path: PathBuf,
    pub content: GenesisJson,
}

impl GenesisJsonWithPath {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, io::Error> {
        let content = fs::read_to_string(path.as_ref())?;
        let content: GenesisJson = serde_json::from_str(&content)?;

        let mut base_path = path.as_ref().to_path_buf();
        base_path.pop();

        Ok(Self { content, base_path })
    }
}

/// The JSON representation of the [Genesis] configuration. This `struct` is used to deserialize
/// the genesis configuration from a JSON file before being converted to a [Genesis] instance.
/// However, this type alone is inadquate for creating the [Genesis] type, for that you have to load
/// the JSON file using [GenesisJsonWithPath] and then convert it to [Genesis] using
/// [`Genesis::try_from<Genesis>`]. This is because the `classes` field of this type contains
/// paths to the class files, which are set to be relative to the JSON file.
#[derive(Debug, serde::Deserialize)]
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
    pub universal_deployer: Option<UniversalDeployerConfigJson>,
    pub allocations: HashMap<ContractAddress, GenesisAccountJson>,
}

#[derive(Debug, thiserror::Error)]
pub enum GenesisTryFromJsonError {
    #[error("Failed to read class file at path {path}: {source}")]
    FileNotFound { source: io::Error, path: PathBuf },
    #[error(transparent)]
    ParsingError(#[from] serde_json::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
    #[error(transparent)]
    ComputeClassHash(#[from] ComputeClassHashError),
    #[error(transparent)]
    ConversionError(#[from] FromByteArrayError),
    #[error(transparent)]
    SierraCompilation(#[from] StarknetSierraCompilationError),
    #[error(transparent)]
    ProgramError(#[from] ProgramError),
    #[error("Missing fee token class: {0}")]
    MissingFeeTokenClass(ClassHash),
    #[error("Missing universal deployer class: {0}")]
    MissingUniversalDeployerClass(ClassHash),
    #[error("Failed to flatten Sierra contract: {0}")]
    Flattened(#[from] JsonError),
}

impl TryFrom<GenesisJsonWithPath> for Genesis {
    type Error = GenesisTryFromJsonError;

    fn try_from(value: GenesisJsonWithPath) -> Result<Self, Self::Error> {
        let GenesisJsonWithPath { content: value, base_path } = value;

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
                                Some(sierra.flatten()?),
                                CompiledContractClass::V1(CompiledContractClassV1::try_from(casm)?),
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

                            (class_hash, class_hash, None, CompiledContractClass::V0(casm))
                        }
                    };

                Ok((class_hash, GenesisClass { compiled_class_hash, sierra, casm }))
            })
            .collect::<Result<_, Self::Error>>()?;

        let fee_token = FeeTokenConfig {
            name: value.fee_token.name,
            symbol: value.fee_token.symbol,
            address: value.fee_token.address,
            decimals: value.fee_token.decimals,
            class_hash: value.fee_token.class.unwrap_or(*DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH),
        };

        match value.fee_token.class {
            Some(hash) => {
                if !classes.contains_key(&hash) {
                    return Err(GenesisTryFromJsonError::MissingFeeTokenClass(hash));
                }
            }

            // if no class hash is provided, use the default fee token class
            None => {
                let _ = classes.insert(
                    *DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH,
                    GenesisClass {
                        sierra: None,
                        casm: DEFAULT_LEGACY_ERC20_CONTRACT_CASM.clone(),
                        compiled_class_hash: *DEFAULT_LEGACY_ERC20_CONTRACT_COMPILED_CLASS_HASH,
                    },
                );
            }
        };

        let universal_deployer = if let Some(config) = value.universal_deployer {
            match config.class {
                Some(hash) => {
                    if !classes.contains_key(&hash) {
                        return Err(GenesisTryFromJsonError::MissingUniversalDeployerClass(hash));
                    }

                    Some(UniversalDeployerConfig {
                        class_hash: hash,
                        address: config.address.unwrap_or(*DEFAULT_UDC_ADDRESS),
                    })
                }

                // if no class hash is provided, use the default UD contract parameters
                None => {
                    let class_hash = *DEFAULT_LEGACY_UDC_CLASS_HASH;
                    let address = config.address.unwrap_or(*DEFAULT_UDC_ADDRESS);

                    let _ = classes.insert(
                        *DEFAULT_LEGACY_UDC_CLASS_HASH,
                        GenesisClass {
                            sierra: None,
                            casm: DEFAULT_LEGACY_UDC_CASM.clone(),
                            compiled_class_hash: *DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH,
                        },
                    );

                    Some(UniversalDeployerConfig { class_hash, address })
                }
            }
        } else {
            None
        };

        let allocations: HashMap<ContractAddress, GenesisAccount> = value
            .allocations
            .into_iter()
            .map(|(address, account)| {
                // check that the class hash exists in the classes field
                let class_hash = match account.class {
                    Some(hash) => {
                        if !classes.contains_key(&hash) {
                            return Err(GenesisTryFromJsonError::MissingFeeTokenClass(hash));
                        } else {
                            hash
                        }
                    }

                    None => {
                        // insert default account class to the classes map
                        let _ = classes.insert(
                            *DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
                            GenesisClass {
                                casm: DEFAULT_OZ_ACCOUNT_CONTRACT_CASM.clone(),
                                sierra: Some(
                                    DEFAULT_OZ_ACCOUNT_CONTRACT.clone().flatten().unwrap(),
                                ),
                                compiled_class_hash:
                                    *DEFAULT_OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH,
                            },
                        );

                        *DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH
                    }
                };

                Ok((
                    address,
                    GenesisAccount {
                        class_hash,
                        nonce: account.nonce,
                        balance: account.balance,
                        storage: account.storage,
                        private_key: account.private_key,
                        public_key: public_key_from_private_key(account.private_key),
                    },
                ))
            })
            .collect::<Result<_, Self::Error>>()?;

        Ok(Self {
            classes,
            fee_token,
            allocations,
            universal_deployer,
            number: value.number,
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

    use starknet::macros::felt;

    use super::{GenesisClassJson, GenesisJson};
    use crate::block::GasPrices;
    use crate::genesis::json::GenesisJsonWithPath;
    use crate::genesis::{
        ContractAddress, FeeTokenConfig, Genesis, GenesisClass, UniversalDeployerConfig,
        DEFAULT_LEGACY_ERC20_CONTRACT_CASM, DEFAULT_LEGACY_UDC_CASM, DEFAULT_LEGACY_UDC_CLASS_HASH,
        DEFAULT_OZ_ACCOUNT_CONTRACT, DEFAULT_OZ_ACCOUNT_CONTRACT_CASM,
        DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
    };

    fn genesis_json() -> GenesisJsonWithPath {
        GenesisJsonWithPath::new("./src/genesis/test-genesis.json").unwrap()
    }

    #[test]
    fn canon() {
        let path = PathBuf::try_from("").unwrap().canonicalize().unwrap();
        println!("{}", path.display())
    }

    #[test]
    fn deserialize_from_json() {
        let genesis: GenesisJson = genesis_json().content;

        assert_eq!(genesis.number, 0);
        assert_eq!(genesis.parent_hash, felt!("0x999"));
        assert_eq!(genesis.timestamp, 5123512314u128);
        assert_eq!(genesis.state_root, felt!("0x99"));
        assert_eq!(genesis.gas_prices.eth, 1241231);
        assert_eq!(genesis.gas_prices.strk, 123123);

        assert_eq!(genesis.fee_token.address, ContractAddress::from(felt!("0x55")));
        assert_eq!(genesis.fee_token.name, String::from("ETHER"));
        assert_eq!(genesis.fee_token.symbol, String::from("ETH"));
        assert_eq!(genesis.fee_token.class, Some(felt!("0x80085")));
        assert_eq!(genesis.fee_token.decimals, 18);

        assert_eq!(
            genesis.universal_deployer.clone().unwrap().address,
            Some(ContractAddress::from(felt!("0x77")))
        );
        assert_eq!(genesis.universal_deployer.unwrap().class, Some(felt!("0x999")));

        let alloc_1 = ContractAddress::from(felt!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"));
        let alloc_2 = ContractAddress::from(felt!("0x70997970C51812dc3A010C7d01b50e0d17dc79C8"));

        assert_eq!(genesis.allocations.len(), 2);
        assert_eq!(genesis.allocations[&alloc_1].private_key, felt!("0x1"));
        assert_eq!(genesis.allocations[&alloc_1].balance, felt!("0xD3C21BCECCEDA1000000"));
        assert_eq!(genesis.allocations[&alloc_1].nonce, Some(felt!("0x1")));
        assert_eq!(genesis.allocations[&alloc_1].class, Some(felt!("0x80085")));
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

    #[test]
    fn genesis_try_from_json() {
        let genesis = genesis_json();
        let actual_genesis = Genesis::try_from(genesis).unwrap();

        let classes = HashMap::from([
            (
                felt!("0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69"),
                GenesisClass {
                    sierra: None,
                    casm: DEFAULT_LEGACY_UDC_CASM.clone(),
                    compiled_class_hash: felt!(
                        "0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69"
                    ),
                },
            ),
            (
                felt!("0x80085"),
                GenesisClass {
                    sierra: None,
                    casm: DEFAULT_LEGACY_UDC_CASM.clone(),
                    compiled_class_hash: felt!("0x80085"),
                },
            ),
            (
                felt!("0x8"),
                GenesisClass {
                    sierra: None,
                    compiled_class_hash: felt!("0x8"),
                    casm: DEFAULT_LEGACY_ERC20_CONTRACT_CASM.clone(),
                },
            ),
            (
                felt!("0x05400e90f7e0ae78bd02c77cd75527280470e2fe19c54970dd79dc37a9d3645c"),
                GenesisClass {
                    compiled_class_hash: felt!(
                        "0x016c6081eb34ad1e0c5513234ed0c025b3c7f305902d291bad534cd6474c85bc"
                    ),
                    casm: DEFAULT_OZ_ACCOUNT_CONTRACT_CASM.clone(),
                    sierra: Some(DEFAULT_OZ_ACCOUNT_CONTRACT.clone().flatten().unwrap()),
                },
            ),
        ]);

        let fee_token = FeeTokenConfig {
            address: ContractAddress::from(felt!("0x55")),
            name: String::from("ETHER"),
            symbol: String::from("ETH"),
            decimals: 18,
            class_hash: felt!("0x8"),
        };

        let allocations = HashMap::from([
            (
                ContractAddress::from(felt!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")),
                crate::genesis::GenesisAccount {
                    private_key: felt!("0x1"),
                    public_key: felt!(
                        "0x01ef15c18599971b7beced415a40f0c7deacfd9b0d1819e03d723d8bc943cfca"
                    ),
                    balance: felt!("0xD3C21BCECCEDA1000000"),
                    nonce: Some(felt!("0x1")),
                    class_hash: felt!("0x80085"),
                    storage: Some(HashMap::from([
                        (felt!("0x1"), felt!("0x1")),
                        (felt!("0x2"), felt!("0x2")),
                    ])),
                },
            ),
            (
                ContractAddress::from(felt!("0x70997970C51812dc3A010C7d01b50e0d17dc79C8")),
                crate::genesis::GenesisAccount {
                    private_key: felt!("0x2"),
                    public_key: felt!(
                        "0x0759ca09377679ecd535a81e83039658bf40959283187c654c5416f439403cf5"
                    ),
                    balance: felt!("0xD3C21BCECCEDA1000000"),
                    class_hash: *DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
                    nonce: None,
                    storage: None,
                },
            ),
        ]);

        let expected_genesis = Genesis {
            classes,
            number: 0,
            fee_token,
            allocations,
            timestamp: 5123512314u128,
            state_root: felt!("0x99"),
            parent_hash: felt!("0x999"),
            gas_prices: GasPrices { eth: 1111, strk: 2222 },
            universal_deployer: Some(UniversalDeployerConfig {
                class_hash: *DEFAULT_LEGACY_UDC_CLASS_HASH,
                address: ContractAddress::from(felt!("0x77")),
            }),
        };

        assert_eq!(actual_genesis.number, expected_genesis.number);
        assert_eq!(actual_genesis.parent_hash, expected_genesis.parent_hash);
        assert_eq!(actual_genesis.timestamp, expected_genesis.timestamp);
        assert_eq!(actual_genesis.state_root, expected_genesis.state_root);
        assert_eq!(actual_genesis.gas_prices, expected_genesis.gas_prices);

        assert_eq!(actual_genesis.fee_token, expected_genesis.fee_token);
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
}
