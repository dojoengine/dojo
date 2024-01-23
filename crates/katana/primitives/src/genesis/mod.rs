use std::collections::HashMap;

use lazy_static::lazy_static;
use rand::rngs::SmallRng;
use rand::{RngCore, SeedableRng};
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::utils::get_contract_address;
use starknet::macros::felt;
use starknet::signers::SigningKey;

use crate::block::{BlockHash, BlockNumber, GasPrices};
use crate::contract::{
    ClassHash, CompiledClassHash, CompiledContractClass, ContractAddress, FlattenedSierraClass,
    SierraClass, StorageKey, StorageValue,
};
use crate::utils::class::{parse_compiled_class, parse_sierra_class};
use crate::FieldElement;

pub mod json;

#[serde_with::serde_as]
#[derive(Debug, Default, Clone, serde::Serialize, PartialEq, Eq)]
pub struct GenesisAccount {
    /// The private key of the account.
    #[serde_as(as = "UfeHex")]
    pub private_key: FieldElement,
    /// The public key associated with the `private_key`.
    #[serde_as(as = "UfeHex")]
    pub public_key: FieldElement,
    /// The class hash of the account contract.
    #[serde_as(as = "UfeHex")]
    pub class_hash: ClassHash,
    /// The amount of the fee token allocated to the account.
    pub balance: FieldElement,
    /// The initial nonce of the account.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<FieldElement>,
    /// The initial storage values of the account.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<HashMap<StorageKey, StorageValue>>,
}

impl GenesisAccount {
    pub fn new(private_key: FieldElement, class_hash: ClassHash) -> (ContractAddress, Self) {
        let public_key = public_key_from_private_key(private_key);
        let address = get_contract_address(
            FieldElement::from(666u32),
            class_hash,
            &[public_key],
            FieldElement::ZERO,
        );

        (
            ContractAddress::from(address),
            Self { private_key, public_key, class_hash, ..Default::default() },
        )
    }

    pub fn new_with_balance(
        private_key: FieldElement,
        class_hash: ClassHash,
        balance: FieldElement,
    ) -> (ContractAddress, Self) {
        let (address, account) = Self::new(private_key, class_hash);
        (address, Self { balance, ..account })
    }
}

#[serde_with::serde_as]
#[derive(Debug, serde::Serialize)]
pub struct FeeTokenConfig {
    /// The name of the fee token.
    pub name: String,
    /// The symbol of the fee token.
    pub symbol: String,
    /// The address of the fee token contract.
    pub address: ContractAddress,
    /// The decimals of the fee token.
    pub decimals: u8,
    // TODO: change to U256
    /// The total supply of the fee token.
    pub total_supply: FieldElement,
    /// The class hash of the fee token contract.
    #[serde_as(as = "UfeHex")]
    pub class_hash: ClassHash,
}

#[serde_with::serde_as]
#[derive(Debug, serde::Serialize, PartialEq, Eq)]
pub struct GenesisClass {
    /// The compiled class hash of the contract class.
    #[serde_as(as = "UfeHex")]
    pub compiled_class_hash: CompiledClassHash,
    /// The casm class definition.
    #[serde(skip_serializing)]
    pub casm: CompiledContractClass,
    /// The sierra class definition.
    #[serde(skip_serializing)]
    pub sierra: Option<FlattenedSierraClass>,
}

#[derive(Debug, serde::Serialize, PartialEq, Eq)]
pub struct UniversalDeployerConfig {
    pub class_hash: ClassHash,
    pub address: ContractAddress,
}

/// Genesis block configuration.
#[serde_with::serde_as]
#[derive(Debug, serde::Serialize)]
pub struct Genesis {
    /// The genesis block parent hash.
    #[serde_as(as = "UfeHex")]
    pub parent_hash: BlockHash,
    /// The genesis block state root.
    #[serde_as(as = "UfeHex")]
    pub state_root: FieldElement,
    /// The genesis block number.
    pub number: BlockNumber,
    /// The genesis block timestamp.
    pub timestamp: u128,
    /// The genesis block L1 gas prices.
    pub gas_prices: GasPrices,
    /// The classes to declare in the genesis block.
    pub classes: HashMap<ClassHash, GenesisClass>,
    /// The fee token configuration.
    pub fee_token: FeeTokenConfig,
    /// The universal deployer (UDC) configuration.
    pub universal_deployer: Option<UniversalDeployerConfig>,
    /// The genesis accounts.
    pub allocations: HashMap<ContractAddress, GenesisAccount>,
}

/// A helper type for allocating accounts in the genesis block.
#[must_use]
pub struct GenesisAllocationsGenerator {
    total: u8,
    seed: [u8; 32],
    balance: FieldElement,
    class_hash: FieldElement,
}

impl GenesisAllocationsGenerator {
    pub fn new(total: u8) -> Self {
        Self { total, seed: [0u8; 32], balance: FieldElement::ZERO, class_hash: Default::default() }
    }

    pub fn new_with_class_hash(self, class_hash: ClassHash) -> Self {
        Self { class_hash, ..self }
    }

    pub fn with_seed(self, seed: [u8; 32]) -> Self {
        Self { seed, ..self }
    }

    pub fn with_balance(self, balance: FieldElement) -> Self {
        Self { balance, ..self }
    }

    /// Generate `total` number of accounts based on the `seed`.
    #[must_use]
    pub fn generate(&self) -> HashMap<ContractAddress, GenesisAccount> {
        let mut seed = self.seed;
        (0..self.total)
            .map(|_| {
                let mut rng = SmallRng::from_seed(seed);
                let mut private_key_bytes = [0u8; 32];

                rng.fill_bytes(&mut private_key_bytes);
                private_key_bytes[0] %= 0x8;
                seed = private_key_bytes;

                let private_key = FieldElement::from_bytes_be(&private_key_bytes).unwrap();
                GenesisAccount::new_with_balance(private_key, self.class_hash, self.balance)
            })
            .collect()
    }
}

fn public_key_from_private_key(private_key: FieldElement) -> FieldElement {
    SigningKey::from_secret_scalar(private_key).verifying_key().scalar()
}

lazy_static! {

    // Pre-compiled contract classes

    pub static ref DEFAULT_UDC_ADDRESS: ContractAddress = ContractAddress(felt!("0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf"));
    pub static ref DEFAULT_FEE_TOKEN_ADDRESS: ContractAddress = ContractAddress(felt!("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7"));

    pub static ref DEFAULT_LEGACY_ERC20_CONTRACT_CASM: CompiledContractClass = parse_compiled_class(include_str!("../../contracts/compiled/erc20.json")).unwrap();
    pub static ref DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH: ClassHash = felt!("0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f");
    pub static ref DEFAULT_LEGACY_ERC20_CONTRACT_COMPILED_CLASS_HASH: CompiledClassHash = felt!("0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f");

    pub static ref DEFAULT_LEGACY_UDC_CASM: CompiledContractClass = parse_compiled_class(include_str!("../../contracts/compiled/universal_deployer.json")).unwrap();
    pub static ref DEFAULT_LEGACY_UDC_CLASS_HASH: ClassHash = felt!("0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69");
    pub static ref DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH: CompiledClassHash = felt!("0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69");

    pub static ref DEFAULT_OZ_ACCOUNT_CONTRACT: SierraClass = parse_sierra_class(include_str!("../../contracts/compiled/oz_account_080.json")).unwrap();
    pub static ref DEFAULT_OZ_ACCOUNT_CONTRACT_CASM: CompiledContractClass = parse_compiled_class(include_str!("../../contracts/compiled/oz_account_080.json")).unwrap();
    pub static ref DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH: ClassHash = felt!("0x05400e90f7e0ae78bd02c77cd75527280470e2fe19c54970dd79dc37a9d3645c");
    pub static ref DEFAULT_OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH: CompiledClassHash = felt!("0x016c6081eb34ad1e0c5513234ed0c025b3c7f305902d291bad534cd6474c85bc");

}
