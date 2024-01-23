use std::collections::HashMap;

use rand::rngs::SmallRng;
use rand::{RngCore, SeedableRng};
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::utils::get_contract_address;
use starknet::signers::SigningKey;

use crate::block::{BlockHash, BlockNumber, GasPrices};
use crate::contract::{
    ClassHash, CompiledClassHash, CompiledContractClass, ContractAddress, FlattenedSierraClass,
    StorageKey, StorageValue,
};
use crate::FieldElement;

pub mod json;

#[serde_with::serde_as]
#[derive(Debug, Default, Clone, serde::Serialize)]
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
    /// The class hash of the fee token contract.
    #[serde_as(as = "UfeHex")]
    pub class_hash: ClassHash,
}

#[serde_with::serde_as]
#[derive(Debug, serde::Serialize)]
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
    /// The genesis accounts.
    pub allocations: HashMap<ContractAddress, GenesisAccount>,
}

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
