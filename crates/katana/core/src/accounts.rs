use std::fmt::Display;
use std::sync::Arc;

use blockifier::abi::abi_utils::get_storage_var_address;
use blockifier::execution::contract_class::ContractClass;
use blockifier::state::state_api::StateResult;
use rand::rngs::SmallRng;
use rand::{RngCore, SeedableRng};
use serde::Serialize;
use serde_with::serde_as;
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::FieldElement;
use starknet::core::utils::get_contract_address;
use starknet::signers::SigningKey;
use starknet_api::core::{ClassHash, CompiledClassHash, ContractAddress, Nonce, PatriciaKey};
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;

use crate::constants::{
    DEFAULT_ACCOUNT_CONTRACT, DEFAULT_ACCOUNT_CONTRACT_CLASS_HASH, FEE_TOKEN_ADDRESS,
};
use crate::db::Database;

#[serde_as]
#[derive(Debug, Clone, Serialize)]
pub struct Account {
    #[serde_as(as = "UfeHex")]
    pub balance: FieldElement,
    #[serde_as(as = "UfeHex")]
    pub public_key: FieldElement,
    #[serde_as(as = "UfeHex")]
    pub private_key: FieldElement,
    #[serde_as(as = "UfeHex")]
    pub address: FieldElement,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    #[serde(skip_serializing)]
    pub contract_class: Arc<ContractClass>,
}

impl Account {
    #[must_use]
    pub fn new(
        private_key: FieldElement,
        balance: FieldElement,
        class_hash: FieldElement,
        contract_class: Arc<ContractClass>,
    ) -> Self {
        let public_key = public_key_from_private_key(private_key);
        let address = get_contract_address(
            FieldElement::from(666u32),
            class_hash,
            &[public_key],
            FieldElement::ZERO,
        );

        Self { address, public_key, balance, class_hash, private_key, contract_class }
    }

    // TODO: separate fund logic from this struct - implement FeeToken type
    pub fn deploy_and_fund(&self, state: &mut dyn Database) -> StateResult<()> {
        self.declare(state)?;
        self.deploy(state)?;
        self.fund(state);
        Ok(())
    }

    fn deploy(&self, state: &mut dyn Database) -> StateResult<()> {
        let address = ContractAddress(patricia_key!(self.address));
        // set the class hash at the account address
        state.set_class_hash_at(address, ClassHash(self.class_hash.into()))?;
        // set the public key in the account contract
        state.set_storage_at(
            address,
            get_storage_var_address("Account_public_key", &[]).unwrap(),
            self.public_key.into(),
        );
        // initialze account nonce
        state.set_nonce(address, Nonce(1u128.into()));
        Ok(())
    }

    fn fund(&self, state: &mut dyn Database) {
        state.set_storage_at(
            ContractAddress(patricia_key!(*FEE_TOKEN_ADDRESS)),
            get_storage_var_address("ERC20_balances", &[self.address.into()]).unwrap(),
            self.balance.into(),
        );
    }

    fn declare(&self, state: &mut dyn Database) -> StateResult<()> {
        let class_hash = ClassHash(self.class_hash.into());

        if state.get_compiled_contract_class(&class_hash).is_ok() {
            return Ok(());
        }

        state.set_contract_class(&class_hash, (*self.contract_class).clone())?;
        state.set_compiled_class_hash(class_hash, CompiledClassHash(self.class_hash.into()))
    }
}

impl Display for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r"
| Account address |  {:#x} 
| Private key     |  {:#x}
| Public key      |  {:#x}",
            self.address, self.private_key, self.public_key
        )
    }
}

pub struct DevAccountGenerator {
    pub total: u8,
    pub seed: [u8; 32],
    pub balance: FieldElement,
    pub class_hash: FieldElement,
    pub contract_class: Arc<ContractClass>,
}

impl DevAccountGenerator {
    #[must_use]
    pub fn new(total: u8) -> Self {
        Self {
            total,
            seed: [0u8; 32],
            balance: FieldElement::ZERO,
            class_hash: (*DEFAULT_ACCOUNT_CONTRACT_CLASS_HASH).into(),
            contract_class: Arc::new((*DEFAULT_ACCOUNT_CONTRACT).clone()),
        }
    }

    pub fn with_seed(self, seed: [u8; 32]) -> Self {
        Self { seed, ..self }
    }

    pub fn with_balance(self, balance: FieldElement) -> Self {
        Self { balance, ..self }
    }

    pub fn with_class(self, class_hash: FieldElement, contract_class: Arc<ContractClass>) -> Self {
        Self { class_hash, contract_class, ..self }
    }

    /// Generate `total` number of accounts based on the `seed`.
    #[must_use]
    pub fn generate(&self) -> Vec<Account> {
        let mut seed = self.seed;
        (0..self.total)
            .map(|_| {
                let mut rng = SmallRng::from_seed(seed);
                let mut private_key_bytes = [0u8; 32];

                rng.fill_bytes(&mut private_key_bytes);
                private_key_bytes[0] %= 0x8;
                seed = private_key_bytes;

                let private_key = FieldElement::from_bytes_be(&private_key_bytes)
                    .expect("able to create FieldElement from bytes");

                Account::new(
                    private_key,
                    self.balance,
                    self.class_hash,
                    self.contract_class.clone(),
                )
            })
            .collect()
    }
}

fn public_key_from_private_key(private_key: FieldElement) -> FieldElement {
    SigningKey::from_secret_scalar(private_key).verifying_key().scalar()
}
