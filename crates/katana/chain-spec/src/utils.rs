use std::cell::{OnceCell, RefCell};
use std::collections::HashSet;
use std::sync::Arc;

use alloy_primitives::U256;
use katana_primitives::class::{ClassHash, ContractClass};
use katana_primitives::contract::{ContractAddress, Nonce};
use katana_primitives::genesis::allocation::{DevGenesisAccount, GenesisAccountAlloc};
use katana_primitives::genesis::constant::{
    DEFAULT_ACCOUNT_CLASS, DEFAULT_LEGACY_ERC20_CLASS, DEFAULT_LEGACY_UDC_CLASS,
    GENESIS_ACCOUNT_CLASS,
};
use katana_primitives::transaction::{
    DeclareTx, DeclareTxV0, DeclareTxV2, DeclareTxWithClass, DeployAccountTx, DeployAccountTxV1,
    ExecutableTx, ExecutableTxWithHash, InvokeTx, InvokeTxV1,
};
use katana_primitives::utils::split_u256;
use katana_primitives::utils::transaction::compute_deploy_account_v1_tx_hash;
use katana_primitives::{felt, Felt};
use num_traits::FromPrimitive;
use starknet::core::utils::{get_contract_address, get_selector_from_name};
use starknet::macros::short_string;
use starknet::signers::SigningKey;

use crate::ChainSpec;

/// A convenience builder for creating valid and executable transactions for the genesis block based
/// on the [`Genesis`].
///
/// The transactions are crafted in a way that can be executed by the StarknetOS Cairo program and thus `blockifier`.
#[derive(Debug)]
pub struct GenesisTransactionsBuilder<'a> {
    chain_spec: &'a mut ChainSpec,
    fee_token: OnceCell<ContractAddress>,
    master_address: OnceCell<ContractAddress>,
    master_signer: SigningKey,
    master_nonce: RefCell<Nonce>,
    transactions: RefCell<Vec<ExecutableTxWithHash>>,
    declared_classes: RefCell<HashSet<ClassHash>>,
}

impl<'a> GenesisTransactionsBuilder<'a> {
    /// Creates a new [`GenesisTransactionsBuilder`] for the given [`ChainSpec`].
    pub fn new(chain_spec: &'a mut ChainSpec) -> Self {
        Self {
            chain_spec,
            fee_token: OnceCell::new(),
            master_address: OnceCell::new(),
            transactions: RefCell::new(Vec::new()),
            master_nonce: RefCell::new(Nonce::ZERO),
            declared_classes: RefCell::new(HashSet::new()),
            master_signer: SigningKey::from_secret_scalar(felt!("0xa55")),
        }
    }

    fn legacy_declare(&self, class: ContractClass) -> ClassHash {
        if matches!(class, ContractClass::Class(..)) {
            panic!("legacy_declare must be called only with legacy class")
        }

        let class = Arc::new(class);
        let class_hash = class.class_hash().unwrap();

        // No need to declare the same class if it was already declared.
        if self.declared_classes.borrow_mut().contains(&class_hash) {
            return class_hash;
        }

        let transaction = ExecutableTx::Declare(DeclareTxWithClass {
            transaction: DeclareTx::V0(DeclareTxV0 {
                sender_address: Felt::ONE.into(),
                chain_id: self.chain_spec.id,
                signature: Vec::new(),
                class_hash,
                max_fee: 0,
            }),
            class,
        });

        let tx_hash = transaction.calculate_hash(false);
        self.declared_classes.borrow_mut().insert(class_hash);
        self.transactions.borrow_mut().push(ExecutableTxWithHash { hash: tx_hash, transaction });

        class_hash
    }

    fn declare(&self, class: ContractClass) -> ClassHash {
        let nonce = self.master_nonce.replace_with(|&mut n| n + Felt::ONE);
        let sender_address = *self.master_address.get().expect("must be initialized first");

        let class_hash = class.class_hash().unwrap();

        // No need to declare the same class if it was already declared.
        if self.declared_classes.borrow_mut().contains(&class_hash) {
            return class_hash;
        }

        let compiled_class_hash = class.clone().compile().unwrap().class_hash().unwrap();

        let mut transaction = DeclareTxV2 {
            chain_id: self.chain_spec.id,
            signature: Vec::new(),
            compiled_class_hash,
            sender_address,
            class_hash,
            max_fee: 0,
            nonce,
        };

        let hash = DeclareTx::V2(transaction.clone()).calculate_hash(false);
        let signature = self.master_signer.sign(&hash).unwrap();
        transaction.signature = vec![signature.r, signature.s];

        self.transactions.borrow_mut().push(ExecutableTxWithHash {
            transaction: ExecutableTx::Declare(DeclareTxWithClass {
                transaction: DeclareTx::V2(transaction),
                class: class.into(),
            }),
            hash,
        });

        self.declared_classes.borrow_mut().insert(class_hash);

        class_hash
    }

    fn deploy(&self, class: ClassHash, ctor_args: Vec<Felt>) -> ContractAddress {
        use std::iter;

        const DEPLOY_CONTRACT_SELECTOR: &str = "deploy_contract";
        let master_address = *self.master_address.get().expect("must be initialized first");

        let salt = Felt::ZERO;
        let contract_address = get_contract_address(salt, class, &ctor_args, Felt::ZERO);

        let ctor_args_len = Felt::from_usize(ctor_args.len()).unwrap();
        let args: Vec<Felt> = iter::once(class) // class_hash
			.chain(iter::once(salt)) // contract_address_salt
			.chain(iter::once(ctor_args_len)) // constructor_calldata_len
			.chain(ctor_args) // constructor_calldata
			.chain(iter::once(Felt::ONE)) // deploy_from_zero
			.collect();

        self.invoke(master_address, DEPLOY_CONTRACT_SELECTOR, args);

        contract_address.into()
    }

    fn invoke(&self, contract: ContractAddress, function: &str, args: Vec<Felt>) {
        use std::iter;

        let nonce = self.master_nonce.replace_with(|&mut n| n + Felt::ONE);
        let sender_address = *self.master_address.get().expect("must be initialized first");
        let selector = get_selector_from_name(function).unwrap();

        let args_len = Felt::from_usize(args.len()).unwrap();
        let calldata: Vec<Felt> = iter::once(Felt::ONE)
            .chain(iter::once(contract.into()))
            .chain(iter::once(selector))
            .chain(iter::once(Felt::ZERO))
            .chain(iter::once(args_len))
            .chain(iter::once(args_len))
            .chain(args)
            .collect();

        let mut transaction = InvokeTxV1 {
            chain_id: self.chain_spec.id,
            signature: Vec::new(),
            sender_address,
            max_fee: 0,
            calldata,
            nonce,
        };

        let tx_hash = InvokeTx::V1(transaction.clone()).calculate_hash(false);
        let signature = self.master_signer.sign(&tx_hash).unwrap();
        transaction.signature = vec![signature.r, signature.s];

        self.transactions.borrow_mut().push(ExecutableTxWithHash {
            transaction: ExecutableTx::Invoke(InvokeTx::V1(transaction)),
            hash: tx_hash,
        });
    }

    fn deploy_predeployed_account(&self, account: &DevGenesisAccount) -> ContractAddress {
        // The salt used in `GenesisAccount::new()` to compute the contract address
        //
        // The only reason we use this value is to make sure the generated account addresses are the same with the previous implementation.
        const SALT: Felt = felt!("666");

        let signer = SigningKey::from_secret_scalar(account.private_key);
        let pubkey = signer.verifying_key().scalar();

        let class_hash = account.class_hash;
        let calldata = vec![pubkey];
        let account_address = get_contract_address(SALT, class_hash, &calldata, Felt::ZERO);

        let tx_hash = compute_deploy_account_v1_tx_hash(
            account_address,
            &calldata,
            class_hash,
            SALT,
            0,
            self.chain_spec.id.into(),
            Felt::ZERO,
            false,
        );

        let signature = signer.sign(&tx_hash).unwrap();

        let transaction = ExecutableTx::DeployAccount(DeployAccountTx::V1(DeployAccountTxV1 {
            signature: vec![signature.r, signature.s],
            contract_address: account_address.into(),
            constructor_calldata: calldata,
            chain_id: self.chain_spec.id,
            contract_address_salt: SALT,
            nonce: Felt::ZERO,
            max_fee: 0,
            class_hash,
        }));

        let tx_hash = transaction.calculate_hash(false);
        self.transactions.borrow_mut().push(ExecutableTxWithHash { hash: tx_hash, transaction });

        account_address.into()
    }

    fn build_master_account(&self) {
        let account_class_hash = self.legacy_declare(GENESIS_ACCOUNT_CLASS.clone());

        let master_pubkey = self.master_signer.verifying_key().scalar();
        let calldata = vec![master_pubkey];
        let salt = Felt::ONE;
        let master_address = get_contract_address(salt, account_class_hash, &calldata, Felt::ZERO);

        self.master_address.set(master_address.into()).expect("must be uninitialized");

        let deploy_account_tx_hash = compute_deploy_account_v1_tx_hash(
            master_address,
            &calldata,
            account_class_hash,
            salt,
            0,
            self.chain_spec.id.into(),
            Felt::ZERO,
            false,
        );

        let signature = self.master_signer.sign(&deploy_account_tx_hash).unwrap();

        let transaction = ExecutableTx::DeployAccount(DeployAccountTx::V1(DeployAccountTxV1 {
            signature: vec![signature.r, signature.s],
            nonce: Felt::ZERO,
            max_fee: 0,
            contract_address_salt: salt,
            contract_address: master_address.into(),
            constructor_calldata: calldata,
            class_hash: account_class_hash,
            chain_id: self.chain_spec.id,
        }));

        let tx_hash = transaction.calculate_hash(false);
        self.transactions.borrow_mut().push(ExecutableTxWithHash { hash: tx_hash, transaction });
        self.master_nonce.replace(Nonce::ONE);
    }

    fn build_core_contracts(&mut self) {
        let udc_class_hash = self.legacy_declare(DEFAULT_LEGACY_UDC_CLASS.clone());
        self.deploy(udc_class_hash, Vec::new());

        let master_address = *self.master_address.get().expect("must be initialized first");

        let ctor_args = vec![
            short_string!("Starknet Token"),
            short_string!("STRK"),
            felt!("0x12"),
            Felt::from_u128(u128::MAX).unwrap(),
            Felt::from_u128(u128::MAX).unwrap(),
            master_address.into(),
        ];

        let erc20_class_hash = self.legacy_declare(DEFAULT_LEGACY_ERC20_CLASS.clone());
        let fee_token_address = self.deploy(erc20_class_hash, ctor_args);

        self.fee_token.set(fee_token_address).expect("must be uninitialized");

        self.chain_spec.fee_contracts.eth = fee_token_address;
        self.chain_spec.fee_contracts.strk = fee_token_address;
    }

    fn build_allocated_dev_accounts(&mut self) {
        let default_account_class_hash = self.declare(DEFAULT_ACCOUNT_CLASS.clone());

        for (expected_addr, account) in self.chain_spec.genesis.accounts() {
            if account.class_hash() != default_account_class_hash {
                panic!(
                    "unexpected account class hash; expected {default_account_class_hash:#x}, got {:#x}",
                    account.class_hash()
                )
            }

            if let GenesisAccountAlloc::DevAccount(account) = account {
                let account_address = self.deploy_predeployed_account(account);
                debug_assert_eq!(&account_address, expected_addr);
                if let Some(amount) = account.balance {
                    self.transfer_balance(account_address, amount);
                }
            }
        }
    }

    fn transfer_balance(&self, recipient: ContractAddress, balance: U256) {
        let fee_token = *self.fee_token.get().expect("must be initialized first");

        let (low_amount, high_amount) = split_u256(balance);
        let args = vec![recipient.into(), low_amount, high_amount];

        const TRANSFER: &str = "transfer";
        self.invoke(fee_token, TRANSFER, args);
    }

    pub fn build(mut self) -> Vec<ExecutableTxWithHash> {
        self.build_master_account();
        self.build_core_contracts();
        self.build_allocated_dev_accounts();
        self.transactions.into_inner()
    }
}
