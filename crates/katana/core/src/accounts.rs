use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use blockifier::abi::abi_utils::get_storage_var_address;
use blockifier::execution::contract_class::{ContractClass, ContractClassV0};
use rand::rngs::SmallRng;
use rand::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use starknet::signers::SigningKey;
use starknet_api::core::{
    calculate_contract_address, ClassHash, CompiledClassHash, ContractAddress, Nonce, PatriciaKey,
};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::transaction::{Calldata, ContractAddressSalt};
use starknet_api::{patricia_key, stark_felt};

use crate::constants::{
    DEFAULT_ACCOUNT_CONTRACT, DEFAULT_ACCOUNT_CONTRACT_CLASS_HASH, FEE_TOKEN_ADDRESS,
};
use crate::state::{ClassRecord, MemDb, StorageRecord};
use crate::util::compute_legacy_class_hash;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Account {
    pub balance: StarkFelt,
    pub class_hash: ClassHash,
    pub public_key: StarkFelt,
    pub private_key: StarkFelt,
    #[serde(rename(serialize = "address"))]
    pub account_address: ContractAddress,
}

impl Account {
    pub fn new(
        balance: StarkFelt,
        public_key: StarkFelt,
        private_key: StarkFelt,
        class_hash: ClassHash,
    ) -> Self {
        let account_address = calculate_contract_address(
            ContractAddressSalt(stark_felt!(666_u128)),
            class_hash,
            &Calldata(Arc::new(vec![public_key])),
            ContractAddress(patricia_key!(0_u8)),
        )
        .expect("should calculate contract address");

        Self { balance, public_key, private_key, class_hash, account_address }
    }

    pub fn deploy(&self, contract_class: &ContractClass, state: &mut MemDb) {
        self.declare(contract_class, state);

        state.state.insert(
            self.account_address,
            StorageRecord {
                // intialize the account nonce
                nonce: Nonce(1u8.into()),
                // set the contract
                class_hash: self.class_hash,
                storage: HashMap::from_iter([
                    // set the public key in the account contract
                    (get_storage_var_address("Account_public_key", &[]).unwrap(), self.public_key),
                ]),
            },
        );

        // set the balance in the FEE CONTRACT
        state.state.entry(ContractAddress(patricia_key!(*FEE_TOKEN_ADDRESS))).and_modify(|r| {
            r.storage.insert(
                get_storage_var_address("ERC20_balances", &[*self.account_address.0.key()])
                    .unwrap(),
                self.balance,
            );
        });
    }

    fn declare(&self, contract_class: &ContractClass, state: &mut MemDb) {
        state.classes.insert(
            self.class_hash,
            ClassRecord {
                class: contract_class.clone(),
                compiled_hash: CompiledClassHash(self.class_hash.0),
                sierra_class: None,
            },
        );
    }
}

#[derive(Debug, Clone)]
pub struct PredeployedAccounts {
    pub seed: [u8; 32],
    pub accounts: Vec<Account>,
    pub initial_balance: StarkFelt,
    pub contract_class: ContractClass,
}

impl PredeployedAccounts {
    pub fn initialize(
        total: u8,
        seed: [u8; 32],
        initial_balance: StarkFelt,
        contract_class_path: Option<PathBuf>,
    ) -> Result<Self> {
        let (class_hash, contract_class) = if let Some(path) = contract_class_path {
            let contract_class_str = fs::read_to_string(path)?;
            let contract_class = serde_json::from_str::<ContractClassV0>(&contract_class_str)
                .expect("can deserialize contract class");
            let class_hash = compute_legacy_class_hash(&contract_class_str)
                .expect("can compute legacy contract class hash");

            (class_hash, ContractClass::V0(contract_class))
        } else {
            Self::default_account_class()
        };

        let accounts = Self::generate_accounts(total, seed, initial_balance, class_hash);

        Ok(Self { seed, accounts, contract_class, initial_balance })
    }

    pub fn deploy_accounts(&self, state: &mut MemDb) {
        for account in &self.accounts {
            account.deploy(&self.contract_class, state);
        }
    }

    pub fn display(&self) -> String {
        fn print_account(account: &Account) -> String {
            format!(
                r"
| Account address |  {} 
| Private key     |  {}
| Public key      |  {}",
                account.account_address.0.key(),
                account.private_key,
                account.public_key
            )
        }

        self.accounts.iter().map(print_account).collect::<Vec<String>>().join("\n")
    }

    fn generate_accounts(
        total: u8,
        seed: [u8; 32],
        balance: StarkFelt,
        class_hash: ClassHash,
    ) -> Vec<Account> {
        let mut seed = seed;
        let mut accounts = vec![];

        for _ in 0..total {
            let mut rng = SmallRng::from_seed(seed);
            let mut private_key_bytes = [0u8; 32];

            rng.fill_bytes(&mut private_key_bytes);
            private_key_bytes[0] %= 0x9;
            seed = private_key_bytes;

            let private_key =
                StarkFelt::new(private_key_bytes).expect("should create StarkFelt from bytes");

            accounts.push(Account::new(
                balance,
                compute_public_key_from_private_key(private_key),
                private_key,
                class_hash,
            ));
        }

        accounts
    }

    pub fn default_account_class() -> (ClassHash, ContractClass) {
        (ClassHash(*DEFAULT_ACCOUNT_CONTRACT_CLASS_HASH), (*DEFAULT_ACCOUNT_CONTRACT).clone())
    }
}

// TODO: remove starknet-rs dependency
fn compute_public_key_from_private_key(private_key: StarkFelt) -> StarkFelt {
    StarkFelt::from(SigningKey::from_secret_scalar(private_key.into()).verifying_key().scalar())
}
