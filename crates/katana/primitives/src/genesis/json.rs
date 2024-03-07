//! JSON representation of the genesis configuration. Used to deserialize the genesis configuration
//! from a JSON file.

use std::collections::{hash_map, BTreeMap, HashMap};
use std::fs::File;
use std::io::{
    BufReader, {self},
};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use base64::prelude::*;
use cairo_lang_starknet::casm_contract_class::StarknetSierraCompilationError;
use cairo_vm::types::errors::program_errors::ProgramError;
use ethers::types::U256;
use rayon::prelude::*;
use serde::de::value::MapAccessDeserializer;
use serde::de::Visitor;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::contract::{ComputeClassHashError, JsonError};
use starknet::core::types::FromByteArrayError;

use super::allocation::{
    DevGenesisAccount, GenesisAccount, GenesisAccountAlloc, GenesisContractAlloc,
};
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
use crate::class::{ClassHash, CompiledClass, SierraClass};
use crate::contract::{ContractAddress, StorageKey, StorageValue};
use crate::genesis::GenesisClass;
use crate::utils::class::{parse_compiled_class_v1, parse_deprecated_compiled_class};
use crate::FieldElement;

type Object = Map<String, Value>;

/// Represents the path to the class artifact or the full JSON artifact itself.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, derive_more::From)]
#[serde(untagged)]
pub enum PathOrFullArtifact {
    /// A path to the file.
    Path(PathBuf),
    /// The full JSON artifact.
    Artifact(Value),
}

impl<'de> Deserialize<'de> for PathOrFullArtifact {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct _Visitor;

        impl<'de> Visitor<'de> for _Visitor {
            type Value = PathOrFullArtifact;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a path to a file or the full json artifact")
            }

            fn visit_str<E>(self, v: &str) -> Result<PathOrFullArtifact, E>
            where
                E: serde::de::Error,
            {
                Ok(PathOrFullArtifact::Path(PathBuf::from(v)))
            }

            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                Ok(PathOrFullArtifact::Artifact(Value::Object(Object::deserialize(
                    MapAccessDeserializer::new(map),
                )?)))
            }
        }

        deserializer.deserialize_any(_Visitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenesisClassJson {
    // pub class: PathBuf,
    pub class: PathOrFullArtifact,
    /// The class hash of the contract. If not provided, the class hash is computed from the
    /// class at `path`.
    pub class_hash: Option<ClassHash>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FeeTokenConfigJson {
    pub name: String,
    pub symbol: String,
    pub address: Option<ContractAddress>,
    pub decimals: u8,
    /// The class hash of the fee token contract.
    /// If not provided, the default fee token class is used.
    pub class: Option<ClassHash>,
    /// To initialize the fee token contract storage
    pub storage: Option<HashMap<StorageKey, StorageValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UniversalDeployerConfigJson {
    /// The address of the universal deployer contract.
    /// If not provided, the default UD address is used.
    pub address: Option<ContractAddress>,
    /// The class hash of the universal deployer contract.
    /// If not provided, the default UD class is used.
    pub class: Option<ClassHash>,
    /// To initialize the UD contract storage
    pub storage: Option<HashMap<StorageKey, StorageValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GenesisContractJson {
    pub class: Option<ClassHash>,
    pub balance: Option<U256>,
    pub nonce: Option<FieldElement>,
    pub storage: Option<HashMap<StorageKey, StorageValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GenesisAccountJson {
    /// The public key of the account.
    pub public_key: FieldElement,
    pub balance: Option<U256>,
    pub nonce: Option<FieldElement>,
    /// The class hash of the account contract. If not provided, the default account class is used.
    pub class: Option<ClassHash>,
    pub storage: Option<HashMap<StorageKey, StorageValue>>,
    pub private_key: Option<FieldElement>,
}

#[derive(Debug, thiserror::Error)]
pub enum GenesisJsonError {
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

    #[error("Unresolved class artifact path {0}")]
    UnresolvedClassPath(PathBuf),

    #[error(transparent)]
    Encode(#[from] base64::EncodeSliceError),

    #[error(transparent)]
    Decode(#[from] base64::DecodeError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

// The JSON representation of the [Genesis] configuration. This `struct` is used to deserialize
/// the genesis configuration from a JSON file before being converted to a [Genesis] instance.
///
/// The JSON format allows specifying either the path to the class artifact or the full artifact
/// embedded directly inside the JSON file. As such, it is required that all paths must be resolved
/// first before converting to [Genesis] using [`Genesis::try_from<GenesisJson>`], otherwise the
/// conversion will fail.
///
/// It is recommended to use [GenesisJson::load] for loading the JSON file as it will resolve
/// the class paths into their actual class artifacts, instead of deserializing it manually
/// (eg, using `serde_json`).
///
/// The path of the class artifact are computed **relative** to the JSON file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

impl GenesisJson {
    /// Load the genesis configuration from a JSON file at the given `path` and resolve all the
    /// class paths to their corresponding class definitions. The paths will be resolved relative
    /// to the JSON file itself.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, GenesisJsonError> {
        let mut path = path.as_ref().to_path_buf();

        let file = File::open(&path)
            .map_err(|source| GenesisJsonError::FileNotFound { path: path.clone(), source })?;

        // Remove the file name from the path to get the base path.
        path.pop();

        let mut genesis: Self = serde_json::from_reader(BufReader::new(file))?;
        // resolves the class paths, if any
        genesis.resolve_class_artifacts(path)?;

        Ok(genesis)
    }

    /// Resolves the paths of the class files to their corresponding class definitions. The
    /// `base_path` is used to calculate the paths of the class files, which are relative to the
    /// JSON file itself.
    ///
    /// This needs to be called if the [GenesisJson] is instantiated without using the
    /// [GenesisJson::load] before converting to [Genesis].
    pub fn resolve_class_artifacts(
        &mut self,
        base_path: impl AsRef<Path>,
    ) -> Result<(), GenesisJsonError> {
        for entry in &mut self.classes {
            if let PathOrFullArtifact::Path(rel_path) = &entry.class {
                let base_path = base_path.as_ref().to_path_buf();
                let artifact = class_artifact_at_path(base_path, rel_path)?;
                entry.class = PathOrFullArtifact::Artifact(artifact);
            }
        }
        Ok(())
    }
}

impl TryFrom<GenesisJson> for Genesis {
    type Error = GenesisJsonError;

    fn try_from(value: GenesisJson) -> Result<Self, Self::Error> {
        let mut classes: HashMap<ClassHash, GenesisClass> = value
            .classes
            .into_par_iter()
            .map(|entry| {
                let GenesisClassJson { class, class_hash } = entry;

                let artifact = match class {
                    PathOrFullArtifact::Artifact(artifact) => artifact,
                    PathOrFullArtifact::Path(path) => {
                        return Err(GenesisJsonError::UnresolvedClassPath(path));
                    }
                };

                let sierra = serde_json::from_value::<SierraClass>(artifact.clone());

                let (class_hash, compiled_class_hash, sierra, casm) = match sierra {
                    Ok(sierra) => {
                        let class = parse_compiled_class_v1(artifact)?;

                        // check if the class hash is provided, otherwise compute it from the
                        // artifacts
                        let class_hash = class_hash.unwrap_or(sierra.class_hash()?);
                        let compiled_hash = class.casm.compiled_class_hash().to_be_bytes();

                        (
                            class_hash,
                            FieldElement::from_bytes_be(&compiled_hash)?,
                            Some(Arc::new(sierra.flatten()?)),
                            Arc::new(CompiledClass::Class(class)),
                        )
                    }

                    // if the artifact is not a sierra contract, we check if it's a legacy contract
                    Err(_) => {
                        let casm = parse_deprecated_compiled_class(artifact.clone())?;

                        let class_hash = if let Some(class_hash) = class_hash {
                            class_hash
                        } else {
                            let casm: LegacyContractClass =
                                serde_json::from_value(artifact.clone())?;
                            casm.class_hash()?
                        };

                        (class_hash, class_hash, None, Arc::new(CompiledClass::Deprecated(casm)))
                    }
                };

                Ok((class_hash, GenesisClass { compiled_class_hash, sierra, casm }))
            })
            .collect::<Result<_, GenesisJsonError>>()?;

        let mut fee_token = FeeTokenConfig {
            name: value.fee_token.name,
            symbol: value.fee_token.symbol,
            total_supply: U256::zero(),
            decimals: value.fee_token.decimals,
            address: value.fee_token.address.unwrap_or(DEFAULT_FEE_TOKEN_ADDRESS),
            class_hash: value.fee_token.class.unwrap_or(DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH),
            storage: value.fee_token.storage,
        };

        match value.fee_token.class {
            Some(hash) => {
                if !classes.contains_key(&hash) {
                    return Err(GenesisJsonError::MissingClass(hash));
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
                        return Err(GenesisJsonError::MissingClass(hash));
                    }

                    Some(UniversalDeployerConfig {
                        class_hash: hash,
                        address: config.address.unwrap_or(DEFAULT_UDC_ADDRESS),
                        storage: config.storage,
                    })
                }

                // if no class hash is provided, use the default UD contract parameters
                None => {
                    let class_hash = DEFAULT_LEGACY_UDC_CLASS_HASH;
                    let address = config.address.unwrap_or(DEFAULT_UDC_ADDRESS);
                    let storage = config.storage;

                    let _ = classes.insert(
                        DEFAULT_LEGACY_UDC_CLASS_HASH,
                        GenesisClass {
                            sierra: None,
                            casm: Arc::new(DEFAULT_LEGACY_UDC_CASM.clone()),
                            compiled_class_hash: DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH,
                        },
                    );

                    Some(UniversalDeployerConfig { class_hash, address, storage })
                }
            }
        } else {
            None
        };

        let mut allocations: BTreeMap<ContractAddress, GenesisAllocation> = BTreeMap::new();

        for (address, account) in value.accounts {
            // check that the class hash exists in the classes field
            let class_hash = match account.class {
                Some(hash) => {
                    if !classes.contains_key(&hash) {
                        return Err(GenesisJsonError::MissingClass(hash));
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

            // increase the total supply of the fee token if balance is given
            if let Some(balance) = account.balance {
                fee_token.total_supply += balance;
            }

            match account.private_key {
                Some(private_key) => allocations.insert(
                    address,
                    GenesisAllocation::Account(GenesisAccountAlloc::DevAccount(
                        DevGenesisAccount {
                            private_key,
                            inner: GenesisAccount {
                                balance: account.balance,
                                class_hash,
                                nonce: account.nonce,
                                storage: account.storage,
                                public_key: account.public_key,
                            },
                        },
                    )),
                ),
                None => allocations.insert(
                    address,
                    GenesisAllocation::Account(GenesisAccountAlloc::Account(GenesisAccount {
                        balance: account.balance,
                        class_hash,
                        nonce: account.nonce,
                        storage: account.storage,
                        public_key: account.public_key,
                    })),
                ),
            };
        }

        for (address, contract) in value.contracts {
            // check that the class hash exists in the classes field
            if let Some(hash) = contract.class {
                if !classes.contains_key(&hash) {
                    return Err(GenesisJsonError::MissingClass(hash));
                }
            }

            // increase the total supply of the fee token if balance is given
            if let Some(balance) = contract.balance {
                fee_token.total_supply += balance;
            }

            allocations.insert(
                address,
                GenesisAllocation::Contract(GenesisContractAlloc {
                    balance: contract.balance,
                    class_hash: contract.class,
                    nonce: contract.nonce,
                    storage: contract.storage,
                }),
            );
        }

        Ok(Genesis {
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

impl FromStr for GenesisJson {
    type Err = GenesisJsonError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s).map_err(GenesisJsonError::from)
    }
}

/// A helper function to conveniently resolve the artifacts in the genesis json if they
/// weren't already resolved and then serialize it to base64 encoding.
///
/// # Arguments
/// * `genesis` - The [GenesisJson] to resolve and serialize.
/// * `base_path` - The base path of the JSON file used to resolve the class artifacts
pub fn resolve_artifacts_and_to_base64<P: AsRef<Path>>(
    mut genesis: GenesisJson,
    base_path: P,
) -> Result<Vec<u8>, GenesisJsonError> {
    genesis.resolve_class_artifacts(base_path)?;
    to_base64(genesis)
}

/// Serialize the [GenesisJson] into base64 encoding.
pub fn to_base64(genesis: GenesisJson) -> Result<Vec<u8>, GenesisJsonError> {
    let data = serde_json::to_vec(&genesis)?;

    // make sure we'll have a slice big enough for base64 + padding
    let mut buf = vec![0; (4 * data.len() / 3) + 4];

    let bytes_written = BASE64_STANDARD.encode_slice(data, &mut buf)?;
    // shorten the buffer to the actual length written
    buf.truncate(bytes_written);

    Ok(buf)
}

/// Deserialize the [GenesisJson] from base64 encoded bytes.
pub fn from_base64(data: &[u8]) -> Result<GenesisJson, GenesisJsonError> {
    let decoded = BASE64_STANDARD.decode(data)?;
    Ok(serde_json::from_slice::<GenesisJson>(&decoded)?)
}

fn class_artifact_at_path(
    base_path: PathBuf,
    relative_path: &PathBuf,
) -> Result<serde_json::Value, GenesisJsonError> {
    let mut path = base_path;
    path.push(relative_path);

    let path =
        path.canonicalize().map_err(|e| GenesisJsonError::FileNotFound { source: e, path })?;

    let file = File::open(&path).map_err(|e| GenesisJsonError::FileNotFound { source: e, path })?;
    let content: Value = serde_json::from_reader(BufReader::new(file))?;

    Ok(content)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::fs::File;
    use std::io::BufReader;
    use std::path::PathBuf;
    use std::str::FromStr;

    use ethers::types::U256;
    use starknet::macros::felt;

    use super::{from_base64, GenesisClassJson, GenesisJson};
    use crate::block::GasPrices;
    use crate::genesis::allocation::{
        DevGenesisAccount, GenesisAccount, GenesisAccountAlloc, GenesisContractAlloc,
    };
    use crate::genesis::constant::{
        DEFAULT_FEE_TOKEN_ADDRESS, DEFAULT_LEGACY_ERC20_CONTRACT_CASM,
        DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH,
        DEFAULT_LEGACY_ERC20_CONTRACT_COMPILED_CLASS_HASH, DEFAULT_LEGACY_UDC_CASM,
        DEFAULT_LEGACY_UDC_CLASS_HASH, DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH,
        DEFAULT_OZ_ACCOUNT_CONTRACT, DEFAULT_OZ_ACCOUNT_CONTRACT_CASM,
        DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH, DEFAULT_OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH,
        DEFAULT_UDC_ADDRESS,
    };
    use crate::genesis::json::to_base64;
    use crate::genesis::{
        ContractAddress, FeeTokenConfig, Genesis, GenesisAllocation, GenesisClass,
        UniversalDeployerConfig,
    };

    #[test]
    fn deserialize_from_json() {
        let file = File::open("./src/genesis/test-genesis.json").unwrap();
        let json: GenesisJson = serde_json::from_reader(file).unwrap();

        assert_eq!(json.number, 0);
        assert_eq!(json.parent_hash, felt!("0x999"));
        assert_eq!(json.timestamp, 5123512314u64);
        assert_eq!(json.state_root, felt!("0x99"));
        assert_eq!(json.gas_prices.eth, 1111);
        assert_eq!(json.gas_prices.strk, 2222);

        assert_eq!(json.fee_token.address, Some(ContractAddress::from(felt!("0x55"))));
        assert_eq!(json.fee_token.name, String::from("ETHER"));
        assert_eq!(json.fee_token.symbol, String::from("ETH"));
        assert_eq!(json.fee_token.class, Some(felt!("0x8")));
        assert_eq!(json.fee_token.decimals, 18);
        assert_eq!(
            json.fee_token.storage,
            Some(HashMap::from([(felt!("0x111"), felt!("0x1")), (felt!("0x222"), felt!("0x2"))]))
        );

        assert_eq!(
            json.universal_deployer.clone().unwrap().address,
            Some(ContractAddress::from(felt!(
                "0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf"
            )))
        );
        assert_eq!(json.universal_deployer.unwrap().class, None);
        assert_eq!(
            json.fee_token.storage,
            Some(HashMap::from([(felt!("0x111"), felt!("0x1")), (felt!("0x222"), felt!("0x2")),]))
        );

        let acc_1 = ContractAddress::from(felt!(
            "0x66efb28ac62686966ae85095ff3a772e014e7fbf56d4c5f6fac5606d4dde23a"
        ));
        let acc_2 = ContractAddress::from(felt!(
            "0x6b86e40118f29ebe393a75469b4d926c7a44c2e2681b6d319520b7c1156d114"
        ));
        let acc_3 = ContractAddress::from(felt!(
            "0x79156ecb3d8f084001bb498c95e37fa1c4b40dbb35a3ae47b77b1ad535edcb9"
        ));
        let acc_4 = ContractAddress::from(felt!(
            "0x053a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf"
        ));

        assert_eq!(json.accounts.len(), 4);

        assert_eq!(json.accounts[&acc_1].public_key, felt!("0x1"));
        assert_eq!(
            json.accounts[&acc_1].balance,
            Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap())
        );
        assert_eq!(json.accounts[&acc_1].nonce, Some(felt!("0x1")));
        assert_eq!(json.accounts[&acc_1].class, Some(felt!("0x80085")));
        assert_eq!(
            json.accounts[&acc_1].storage,
            Some(HashMap::from([(felt!("0x1"), felt!("0x1")), (felt!("0x2"), felt!("0x2")),]))
        );

        assert_eq!(json.accounts[&acc_2].public_key, felt!("0x2"));
        assert_eq!(
            json.accounts[&acc_2].balance,
            Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap())
        );
        assert_eq!(json.accounts[&acc_2].nonce, None);
        assert_eq!(json.accounts[&acc_2].class, None);
        assert_eq!(json.accounts[&acc_2].storage, None);

        assert_eq!(json.accounts[&acc_3].public_key, felt!("0x3"));
        assert_eq!(json.accounts[&acc_3].balance, None);
        assert_eq!(json.accounts[&acc_3].nonce, None);
        assert_eq!(json.accounts[&acc_3].class, None);
        assert_eq!(json.accounts[&acc_3].storage, None);

        assert_eq!(json.accounts[&acc_4].public_key, felt!("0x4"));
        assert_eq!(json.accounts[&acc_4].private_key.unwrap(), felt!("0x115"));
        assert_eq!(
            json.accounts[&acc_4].balance,
            Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap())
        );
        assert_eq!(json.accounts[&acc_4].nonce, None);
        assert_eq!(json.accounts[&acc_4].class, None);
        assert_eq!(json.accounts[&acc_4].storage, None);

        assert_eq!(json.contracts.len(), 3);

        let contract_1 = ContractAddress::from(felt!(
            "0x29873c310fbefde666dc32a1554fea6bb45eecc84f680f8a2b0a8fbb8cb89af"
        ));
        let contract_2 = ContractAddress::from(felt!(
            "0xe29882a1fcba1e7e10cad46212257fea5c752a4f9b1b1ec683c503a2cf5c8a"
        ));
        let contract_3 = ContractAddress::from(felt!(
            "0x05400e90f7e0ae78bd02c77cd75527280470e2fe19c54970dd79dc37a9d3645c"
        ));

        assert_eq!(
            json.contracts[&contract_1].balance,
            Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap())
        );
        assert_eq!(json.contracts[&contract_1].nonce, None);
        assert_eq!(json.contracts[&contract_1].class, Some(felt!("0x8")));
        assert_eq!(
            json.contracts[&contract_1].storage,
            Some(HashMap::from([(felt!("0x1"), felt!("0x1")), (felt!("0x2"), felt!("0x2"))]))
        );

        assert_eq!(
            json.contracts[&contract_2].balance,
            Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap())
        );
        assert_eq!(json.contracts[&contract_2].nonce, None);
        assert_eq!(json.contracts[&contract_2].class, None);
        assert_eq!(json.contracts[&contract_2].storage, None);

        assert_eq!(json.contracts[&contract_3].balance, None);
        assert_eq!(json.contracts[&contract_3].nonce, None);
        assert_eq!(json.contracts[&contract_3].class, None);
        assert_eq!(
            json.contracts[&contract_3].storage,
            Some(HashMap::from([(felt!("0x1"), felt!("0x1"))]))
        );

        assert_eq!(
            json.classes,
            vec![
                GenesisClassJson {
                    class_hash: Some(felt!("0x8")),
                    class: PathBuf::from("../../contracts/compiled/erc20.json").into(),
                },
                GenesisClassJson {
                    class_hash: Some(felt!("0x80085")),
                    class: PathBuf::from("../../contracts/compiled/universal_deployer.json").into(),
                },
                GenesisClassJson {
                    class_hash: Some(felt!("0xa55")),
                    class: PathBuf::from("../../contracts/compiled/oz_account_080.json").into(),
                },
            ]
        );
    }

    #[test]
    fn deserialize_from_json_with_class() {
        let file = File::open("./src/genesis/test-genesis-with-class.json").unwrap();
        let genesis: GenesisJson = serde_json::from_reader(BufReader::new(file)).unwrap();

        assert_eq!(
            genesis.classes,
            vec![
                GenesisClassJson {
                    class_hash: Some(felt!("0x8")),
                    class: PathBuf::from("../../contracts/compiled/erc20.json").into(),
                },
                GenesisClassJson {
                    class_hash: Some(felt!("0x80085")),
                    class: PathBuf::from("../../contracts/compiled/universal_deployer.json").into(),
                },
                GenesisClassJson {
                    class_hash: Some(felt!("0xa55")),
                    class: serde_json::to_value(DEFAULT_OZ_ACCOUNT_CONTRACT.clone())
                        .unwrap()
                        .into(),
                },
            ]
        );
    }

    #[test]
    fn genesis_load_from_json() {
        let path = PathBuf::from("./src/genesis/test-genesis.json");

        let json = GenesisJson::load(path).unwrap();
        let actual_genesis = Genesis::try_from(json).unwrap();

        let expected_classes = HashMap::from([
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

        let expected_fee_token = FeeTokenConfig {
            address: ContractAddress::from(felt!("0x55")),
            name: String::from("ETHER"),
            symbol: String::from("ETH"),
            total_supply: U256::from_str("0xD3C21BCECCEDA1000000").unwrap() * 5,
            decimals: 18,
            class_hash: felt!("0x8"),
            storage: Some(HashMap::from([
                (felt!("0x111"), felt!("0x1")),
                (felt!("0x222"), felt!("0x2")),
            ])),
        };

        let acc_1 = ContractAddress::from(felt!(
            "0x66efb28ac62686966ae85095ff3a772e014e7fbf56d4c5f6fac5606d4dde23a"
        ));
        let acc_2 = ContractAddress::from(felt!(
            "0x6b86e40118f29ebe393a75469b4d926c7a44c2e2681b6d319520b7c1156d114"
        ));
        let acc_3 = ContractAddress::from(felt!(
            "0x79156ecb3d8f084001bb498c95e37fa1c4b40dbb35a3ae47b77b1ad535edcb9"
        ));
        let acc_4 = ContractAddress::from(felt!(
            "0x053a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf"
        ));
        let contract_1 = ContractAddress::from(felt!(
            "0x29873c310fbefde666dc32a1554fea6bb45eecc84f680f8a2b0a8fbb8cb89af"
        ));
        let contract_2 = ContractAddress::from(felt!(
            "0xe29882a1fcba1e7e10cad46212257fea5c752a4f9b1b1ec683c503a2cf5c8a"
        ));
        let contract_3 = ContractAddress::from(felt!(
            "0x05400e90f7e0ae78bd02c77cd75527280470e2fe19c54970dd79dc37a9d3645c"
        ));

        let expected_allocations = BTreeMap::from([
            (
                acc_1,
                GenesisAllocation::Account(GenesisAccountAlloc::Account(GenesisAccount {
                    public_key: felt!("0x1"),
                    balance: Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap()),
                    nonce: Some(felt!("0x1")),
                    class_hash: felt!("0x80085"),
                    storage: Some(HashMap::from([
                        (felt!("0x1"), felt!("0x1")),
                        (felt!("0x2"), felt!("0x2")),
                    ])),
                })),
            ),
            (
                acc_2,
                GenesisAllocation::Account(GenesisAccountAlloc::Account(GenesisAccount {
                    public_key: felt!("0x2"),
                    balance: Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap()),
                    class_hash: DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
                    nonce: None,
                    storage: None,
                })),
            ),
            (
                acc_3,
                GenesisAllocation::Account(GenesisAccountAlloc::Account(GenesisAccount {
                    public_key: felt!("0x3"),
                    balance: None,
                    class_hash: DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
                    nonce: None,
                    storage: None,
                })),
            ),
            (
                acc_4,
                GenesisAllocation::Account(GenesisAccountAlloc::DevAccount(DevGenesisAccount {
                    private_key: felt!("0x115"),
                    inner: GenesisAccount {
                        public_key: felt!("0x4"),
                        balance: Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap()),
                        class_hash: DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
                        nonce: None,
                        storage: None,
                    },
                })),
            ),
            (
                contract_1,
                GenesisAllocation::Contract(GenesisContractAlloc {
                    balance: Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap()),
                    nonce: None,
                    class_hash: Some(felt!("0x8")),
                    storage: Some(HashMap::from([
                        (felt!("0x1"), felt!("0x1")),
                        (felt!("0x2"), felt!("0x2")),
                    ])),
                }),
            ),
            (
                contract_2,
                GenesisAllocation::Contract(GenesisContractAlloc {
                    balance: Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap()),
                    nonce: None,
                    class_hash: None,
                    storage: None,
                }),
            ),
            (
                contract_3,
                GenesisAllocation::Contract(GenesisContractAlloc {
                    balance: None,
                    nonce: None,
                    class_hash: None,
                    storage: Some(HashMap::from([(felt!("0x1"), felt!("0x1"))])),
                }),
            ),
        ]);

        let expected_genesis = Genesis {
            classes: expected_classes,
            number: 0,
            fee_token: expected_fee_token,
            allocations: expected_allocations,
            timestamp: 5123512314u64,
            sequencer_address: ContractAddress::from(felt!("0x100")),
            state_root: felt!("0x99"),
            parent_hash: felt!("0x999"),
            gas_prices: GasPrices { eth: 1111, strk: 2222 },
            universal_deployer: Some(UniversalDeployerConfig {
                class_hash: DEFAULT_LEGACY_UDC_CLASS_HASH,
                address: ContractAddress::from(felt!(
                    "0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf"
                )),
                storage: Some([(felt!("0x10"), felt!("0x100"))].into()),
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
                "0x66efb28ac62686966ae85095ff3a772e014e7fbf56d4c5f6fac5606d4dde23a": {
                    "publicKey": "0x1",
                    "balance": "0xD3C21BCECCEDA1000000"
                }
            },
            "contracts": {},
            "classes": []
        }
        "#;

        let genesis_json: GenesisJson = GenesisJson::from_str(json).unwrap();
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
            storage: None,
        };

        let allocations = BTreeMap::from([(
            ContractAddress::from(felt!(
                "0x66efb28ac62686966ae85095ff3a772e014e7fbf56d4c5f6fac5606d4dde23a"
            )),
            GenesisAllocation::Account(GenesisAccountAlloc::Account(GenesisAccount {
                public_key: felt!("0x1"),
                balance: Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap()),
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
                storage: None,
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

    #[test]
    fn genesis_from_json_with_unresolved_paths() {
        let file = File::open("./src/genesis/test-genesis.json").unwrap();
        let json: GenesisJson = serde_json::from_reader(file).unwrap();
        assert!(
            Genesis::try_from(json)
                .unwrap_err()
                .to_string()
                .contains("Unresolved class artifact path")
        );
    }

    #[test]
    fn encode_decode_genesis_file_to_base64() {
        let path = PathBuf::from("./src/genesis/test-genesis.json");

        let genesis = GenesisJson::load(path).unwrap();
        let genesis_clone = genesis.clone();

        let encoded = to_base64(genesis_clone).unwrap();
        let decoded = from_base64(encoded.as_slice()).unwrap();

        assert_eq!(genesis, decoded);
    }
}
