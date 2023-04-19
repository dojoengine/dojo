use std::sync::Mutex;

use crate::{
    block_context::Base,
    state::{DictStateReader, ACCOUNT_CONTRACT_CLASS_HASH},
    util::deploy_account_tx,
};
use blockifier::{
    abi::abi_utils::get_storage_var_address,
    block_context::BlockContext,
    state::{cached_state::CachedState, state_api::State},
    transaction::{account_transaction::AccountTransaction, transactions::ExecutableTransaction},
};
use starknet_api::{
    core::ContractAddress,
    hash::StarkFelt,
    stark_felt,
    transaction::{ContractAddressSalt, Fee},
};

pub struct KatanaSequencer {
    pub block_context: BlockContext,
    pub state: Mutex<CachedState<DictStateReader>>,
}

impl KatanaSequencer {
    pub fn new() -> Self {
        Self {
            block_context: BlockContext::base(),
            state: Mutex::new(CachedState::new(DictStateReader::new())),
        }
    }

    pub fn deploy_account(
        &mut self,
        contract_address_salt: ContractAddressSalt,
        balance: u64,
    ) -> anyhow::Result<ContractAddress> {
        let max_fee = Fee(u128::from(balance));
        let deploy_account_tx =
            deploy_account_tx(ACCOUNT_CONTRACT_CLASS_HASH, contract_address_salt, max_fee);
        let deployed_account_address = deploy_account_tx.contract_address;

        let deployed_account_balance_key =
            get_storage_var_address("ERC20_balances", &[*deployed_account_address.0.key()])
                .unwrap();
        self.state.lock().unwrap().set_storage_at(
            self.block_context.fee_token_address,
            deployed_account_balance_key,
            stark_felt!(Fee(u128::from(balance)).0 as u64),
        );

        let account_tx = AccountTransaction::DeployAccount(deploy_account_tx);
        account_tx.execute(&mut self.state.lock().unwrap(), &self.block_context)?;
        Ok(deployed_account_address)
    }
}

impl Default for KatanaSequencer {
    fn default() -> Self {
        Self::new()
    }
}
