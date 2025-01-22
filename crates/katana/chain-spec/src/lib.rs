use std::cell::{OnceCell, RefCell};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use alloy_primitives::U256;
use anyhow::{Context, Result};
use katana_primitives::block::{ExecutableBlock, PartialHeader};
use katana_primitives::chain::ChainId;
use katana_primitives::class::{ClassHash, ContractClass};
use katana_primitives::contract::{ContractAddress, Nonce};
use katana_primitives::da::L1DataAvailabilityMode;
use katana_primitives::genesis::allocation::{
    DevAllocationsGenerator, DevGenesisAccount, GenesisAccountAlloc,
};
use katana_primitives::genesis::constant::{
    DEFAULT_ACCOUNT_CLASS, DEFAULT_ETH_FEE_TOKEN_ADDRESS, DEFAULT_LEGACY_ERC20_CLASS,
    DEFAULT_LEGACY_UDC_CLASS, DEFAULT_PREFUNDED_ACCOUNT_BALANCE, DEFAULT_STRK_FEE_TOKEN_ADDRESS,
    GENESIS_ACCOUNT_CLASS,
};
use katana_primitives::genesis::json::GenesisJson;
use katana_primitives::genesis::Genesis;
use katana_primitives::transaction::{
    DeclareTx, DeclareTxV0, DeclareTxV2, DeclareTxWithClass, DeployAccountTx, DeployAccountTxV1,
    ExecutableTx, ExecutableTxWithHash, InvokeTx, InvokeTxV1,
};
use katana_primitives::utils::split_u256;
use katana_primitives::utils::transaction::compute_deploy_account_v1_tx_hash;
use katana_primitives::version::CURRENT_STARKNET_VERSION;
use katana_primitives::{eth, felt, Felt};
use lazy_static::lazy_static;
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};
use starknet::core::utils::{get_contract_address, get_selector_from_name};
use starknet::macros::short_string;
use starknet::signers::SigningKey;
use url::Url;

/// The rollup chain specification.
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ChainSpec {
    /// The rollup network chain id.
    pub id: ChainId,

    /// The chain's genesis states.
    pub genesis: Genesis,

    /// The chain fee token contract.
    pub fee_contracts: FeeContracts,

    /// The chain's settlement layer configurations.
    ///
    /// This should only be optional if the chain is in development mode.
    pub settlement: Option<SettlementLayer>,
}

/// Tokens that can be used for transaction fee payments in the chain. As
/// supported on Starknet.
// TODO: include both l1 and l2 addresses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct FeeContracts {
    /// L2 ETH fee token address. Used for paying pre-V3 transactions.
    pub eth: ContractAddress,
    /// L2 STRK fee token address. Used for paying V3 transactions.
    pub strk: ContractAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SettlementLayer {
    Ethereum {
        // The id of the settlement chain.
        id: eth::ChainId,

        // url for ethereum rpc provider
        rpc_url: Url,

        /// account on the ethereum network
        account: eth::Address,

        // - The core appchain contract used to settlement
        core_contract: eth::Address,
    },

    Starknet {
        // The id of the settlement chain.
        id: ChainId,

        // url for starknet rpc provider
        rpc_url: Url,

        /// account on the starknet network
        account: ContractAddress,

        // - The core appchain contract used to settlement
        core_contract: ContractAddress,
    },
}

//////////////////////////////////////////////////////////////
// 	ChainSpec implementations
//////////////////////////////////////////////////////////////

impl ChainSpec {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(&path)?;
        let cs = serde_json::from_str::<ChainSpecFile>(&content)?;

        let file = File::open(&cs.genesis).context("failed to open genesis file")?;

        // the genesis file is stored as its JSON representation
        let genesis_json: GenesisJson = serde_json::from_reader(BufReader::new(file))?;
        let genesis = Genesis::try_from(genesis_json)?;

        Ok(Self { genesis, id: cs.id, settlement: cs.settlement, fee_contracts: cs.fee_contracts })
    }

    pub fn store<P: AsRef<Path>>(self, path: P) -> anyhow::Result<()> {
        let cfg_path = path.as_ref();
        let mut genesis_path = cfg_path.to_path_buf();
        genesis_path.set_file_name("genesis.json");

        let stored = ChainSpecFile {
            id: self.id,
            genesis: genesis_path,
            settlement: self.settlement,
            fee_contracts: self.fee_contracts,
        };

        // convert the genesis to its JSON representation and store it
        let genesis_json = GenesisJson::try_from(self.genesis)?;

        serde_json::to_writer_pretty(File::create(cfg_path)?, &stored)?;
        serde_json::to_writer_pretty(File::create(stored.genesis)?, &genesis_json)?;

        Ok(())
    }

    pub fn block(&mut self) -> ExecutableBlock {
        let header = PartialHeader {
            protocol_version: CURRENT_STARKNET_VERSION,
            number: self.genesis.number,
            timestamp: self.genesis.timestamp,
            parent_hash: self.genesis.parent_hash,
            l1_da_mode: L1DataAvailabilityMode::Calldata,
            l1_gas_prices: self.genesis.gas_prices.clone(),
            l1_data_gas_prices: self.genesis.gas_prices.clone(),
            sequencer_address: self.genesis.sequencer_address,
        };

        let transactions = GenesisTransactionsBuilder::new(self).build();

        ExecutableBlock { header, body: transactions }
    }
}

impl Default for ChainSpec {
    fn default() -> Self {
        DEV.clone()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ChainSpecFile {
    id: ChainId,
    fee_contracts: FeeContracts,
    #[serde(skip_serializing_if = "Option::is_none")]
    settlement: Option<SettlementLayer>,
    genesis: PathBuf,
}

lazy_static! {
    /// The default chain specification in dev mode.
    pub static ref DEV: ChainSpec = {
        let mut chain_spec = DEV_UNALLOCATED.clone();

        let accounts = DevAllocationsGenerator::new(10)
            .with_balance(U256::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE))
            .generate();

        chain_spec.genesis.extend_allocations(accounts.into_iter().map(|(k, v)| (k, v.into())));
        chain_spec
    };

    /// The default chain specification for dev mode but without any allocations.
    ///
    /// Used when we want to create a chain spec with user defined # of allocations.
    pub static ref DEV_UNALLOCATED: ChainSpec = {
        let id = ChainId::parse("KATANA").unwrap();
        let genesis = Genesis::default();
        let fee_contracts = FeeContracts { eth: DEFAULT_ETH_FEE_TOKEN_ADDRESS, strk: DEFAULT_STRK_FEE_TOKEN_ADDRESS };
        ChainSpec { id, genesis, fee_contracts, settlement: None }
    };
}

struct GenesisTransactionsBuilder<'a> {
    chain_spec: &'a mut ChainSpec,
    master_address: OnceCell<ContractAddress>,
    master_signer: SigningKey,
    fee_token: OnceCell<ContractAddress>,
    transactions: RefCell<Vec<ExecutableTxWithHash>>,
    master_nonce: RefCell<Nonce>,
}

impl<'a> GenesisTransactionsBuilder<'a> {
    fn new(chain_spec: &'a mut ChainSpec) -> Self {
        Self {
            chain_spec,
            fee_token: OnceCell::new(),
            master_address: OnceCell::new(),
            transactions: RefCell::new(Vec::new()),
            master_nonce: RefCell::new(Nonce::ZERO),
            master_signer: SigningKey::from_secret_scalar(felt!("0xa55")),
        }
    }

    fn legacy_declare(&self, class: ContractClass) -> ClassHash {
        if matches!(class, ContractClass::Class(..)) {
            panic!("genesis declare can only support legacy classes")
        }

        let class = Arc::new(class);
        let class_hash = class.class_hash().unwrap();

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

        let tx_hash = transaction.compute_hash(false);
        self.transactions.borrow_mut().push(ExecutableTxWithHash { hash: tx_hash, transaction });

        class_hash
    }

    fn declare(&self, class: ContractClass) -> ClassHash {
        let nonce = self.master_nonce.replace_with(|&mut n| n + Felt::ONE);
        let sender_address = *self.master_address.get().expect("must be initialized");

        let class_hash = class.class_hash().unwrap();
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

        class_hash
    }

    fn deploy(&self, class: ClassHash, ctor_args: Vec<Felt>) -> ContractAddress {
        use std::iter;

        const DEPLOY_CONTRACT_SELECTOR: &str = "deploy_contract";
        let master_address = *self.master_address.get().expect("must be initialized");

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
        let sender_address = *self.master_address.get().expect("must be initialized");
        let selector = get_selector_from_name(function).expect("valid function selector");

        let args_len = Felt::from_usize(args.len()).unwrap();
        let calldata: Vec<Felt> = iter::once(Felt::ONE)
	        // --- call array arguments
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

        // deploy account tx

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

        let tx_hash = transaction.compute_hash(false);
        self.transactions.borrow_mut().push(ExecutableTxWithHash { hash: tx_hash, transaction });

        account_address.into()
    }

    fn build_master_account(&self) {
        // Declare master account class
        let account_class_hash = self.legacy_declare(GENESIS_ACCOUNT_CLASS.clone());

        // Deploy master account
        let master_pubkey = self.master_signer.verifying_key().scalar();
        let calldata = vec![master_pubkey];
        let salt = Felt::ONE;
        let master_address = get_contract_address(salt, account_class_hash, &calldata, Felt::ZERO);

        self.master_address.set(master_address.into()).expect("must be empty");

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

        let tx_hash = transaction.compute_hash(false);
        self.transactions.borrow_mut().push(ExecutableTxWithHash { hash: tx_hash, transaction });
        self.master_nonce.replace(Nonce::ONE);
    }

    fn build_core_contracts(&mut self) {
        // udc class declare
        {
            let udc_class_hash = self.legacy_declare(DEFAULT_LEGACY_UDC_CLASS.clone());
            self.deploy(udc_class_hash, Vec::new());
        }

        // fee token's class declare
        {
            let master_address = *self.master_address.get().expect("must be initialized");

            let ctor_args = vec![
                short_string!("Starknet Token"),     // ERC20 name
                short_string!("STRK"),               // ERC20 symbol
                felt!("0x12"),                       // ERC20 decimals
                Felt::from_u128(u128::MAX).unwrap(), // ERC20 total supply (low)
                Felt::from_u128(u128::MAX).unwrap(), // ERC20 total supply (high)
                master_address.into(),               // recipient
            ];

            let erc20_class_hash = self.legacy_declare(DEFAULT_LEGACY_ERC20_CLASS.clone());
            let fee_token_address = self.deploy(erc20_class_hash, ctor_args);

            self.fee_token.set(fee_token_address).expect("must be empty");

            // update the chain spec so that the execution context is usind the correct fee token
            // address
            self.chain_spec.fee_contracts.eth = fee_token_address;
            self.chain_spec.fee_contracts.strk = fee_token_address;
        }
    }

    fn build_allocated_accounts(&mut self) {
        let default_account_class_hash = self.declare(DEFAULT_ACCOUNT_CLASS.clone());

        for (expected_addr, account) in self.chain_spec.genesis.accounts() {
            if account.class_hash() != default_account_class_hash {
                panic!("Unexpected account class hash");
            }

            if let GenesisAccountAlloc::DevAccount(account) = account {
                let account_address = self.deploy_predeployed_account(account);
                debug_assert_eq!(&account_address, expected_addr);
                // transfer token from master account to the current account
                if let Some(amount) = account.balance {
                    self.transfer_balance(account_address, amount);
                }
            }
        }
    }

    // This transfer balances from the master account to the given recipient address
    //
    // Make sure to deploy the fee token first before calling this function.
    fn transfer_balance(&self, recipient: ContractAddress, balance: U256) {
        let fee_token = *self.fee_token.get().expect("must not be empty");

        let (low_amount, high_amount) = split_u256(balance);
        let args = vec![recipient.into(), low_amount, high_amount];

        const TRANSFER: &str = "transfer";
        self.invoke(fee_token, TRANSFER, args);
    }

    pub fn build(mut self) -> Vec<ExecutableTxWithHash> {
        self.build_master_account();
        self.build_core_contracts();
        self.build_allocated_accounts();
        self.transactions.into_inner()
    }
}
