use blockifier::block_context::BlockContext;
use blockifier::execution::contract_class::{ContractClass, ContractClassV0, ContractClassV1};
use blockifier::state::cached_state::CachedState;
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transactions::ExecutableTransaction;
use starknet_api::api_core::{ClassHash, ContractAddress, Nonce};
use starknet_api::hash::StarkFelt;
use starknet_api::state::StorageKey;

use crate::utils::{
    addr, block_context, HashMap, TransactionExecutionError, TransactionExecutionInfo,
    ACCOUNT_ADDR, FEE_TKN_ADDR,
};
use crate::ClientState;

pub struct Client {
    cache: CachedState<ClientState>,
    block_ctx: BlockContext,
}

#[derive(Debug)]
pub enum ClientErr {
    UndeclaredClassHash(ClassHash),
    DuplicateContract(ContractAddress),
    DuplicateClassHash(ClassHash),
    Msg(String),
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    pub fn new() -> Client {
        let state = ClientState::default();

        let mut client = Client { cache: CachedState::from(state), block_ctx: block_context() };

        let account_json = include_bytes!("../contracts/account_without_validation.json");
        let account_json = String::from_utf8_lossy(account_json);
        let erc20_json = include_bytes!("../contracts/erc20.json");
        let erc20_json = String::from_utf8_lossy(erc20_json);

        client.register_class_v0(FEE_TKN_ADDR, &erc20_json).unwrap();
        client.register_class_v0(ACCOUNT_ADDR, &account_json).unwrap();

        let fees_contract_storage = HashMap::from([
            (addr::storage("ERC20_balances", &[ACCOUNT_ADDR]), "200000000000000"),
            (addr::storage("permitted_minter", &[]), ACCOUNT_ADDR),
        ]);

        client.register_contract(FEE_TKN_ADDR, FEE_TKN_ADDR, fees_contract_storage).unwrap();
        client.register_contract(ACCOUNT_ADDR, ACCOUNT_ADDR, HashMap::new()).unwrap();

        client
    }

    // pub fn cache(&mut self) -> &mut CachedState<ClientState> {
    //     &mut self.cache.count_actual_state_changes()
    // }

    pub fn state(&mut self) -> &mut ClientState {
        &mut self.cache.state
    }

    fn classes(&mut self) -> &mut HashMap<ClassHash, ContractClass> {
        &mut self.cache.state.classes
    }

    fn contracts(
        &mut self,
    ) -> &mut HashMap<ContractAddress, (ClassHash, Nonce, HashMap<StorageKey, StarkFelt>)> {
        &mut self.cache.state.contracts
    }

    pub fn register_class(&mut self, hash: &str, json: &str) -> Result<(), ClientErr> {
        let hash = addr::class(hash);
        if self.classes().get(&hash).is_some() {
            return Err(ClientErr::DuplicateClassHash(hash));
        }
        let contract_class = ContractClassV1::try_from_json_string(json);
        if contract_class.is_err() {
            return Err(ClientErr::Msg(format!("{:?}", contract_class)));
        }
        self.classes().insert(hash, ContractClass::V1(contract_class.unwrap()));

        Ok(())
    }

    pub fn register_class_v0(&mut self, hash: &str, json: &str) -> Result<(), ClientErr> {
        let hash = addr::class(hash);
        if self.classes().get(&hash).is_some() {
            return Err(ClientErr::DuplicateClassHash(hash));
        }
        let contract_class = ContractClassV0::try_from_json_string(json);
        if contract_class.is_err() {
            return Err(ClientErr::Msg("Couldn't parse JSON as ContractClassV0.".into()));
        }
        self.classes().insert(hash, ContractClass::V0(contract_class.unwrap()));

        Ok(())
    }

    pub fn register_contract(
        &mut self,
        address: &str,
        class_hash: &str,
        storage: HashMap<StorageKey, &str>,
    ) -> Result<(), ClientErr> {
        let address = addr::contract(address);
        let class_hash = addr::class(class_hash);
        if self.contracts().get(&address).is_some() {
            return Err(ClientErr::DuplicateContract(address));
        }
        if self.classes().get(&class_hash).is_none() {
            return Err(ClientErr::UndeclaredClassHash(class_hash));
        }
        self.contracts().insert(
            address,
            (
                class_hash,
                Nonce(1_u8.into()),
                storage.iter().map(|r| (*r.0, addr::felt(*r.1))).collect(),
            ),
        );
        Ok(())
    }

    pub fn execute(
        &mut self,
        tx: AccountTransaction,
    ) -> Result<TransactionExecutionInfo, TransactionExecutionError> {
        tx.execute(&mut self.cache, &self.block_ctx, false, false)
    }

    pub fn update_storage(_storage: HashMap<ContractAddress, HashMap<StorageKey, StarkFelt>>) {
        todo!()
    }
}
