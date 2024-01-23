use katana_core::accounts::Account;
use katana_core::backend::config::Environment;
use katana_primitives::chain::ChainId;
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::macros::felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::{LocalWallet, SigningKey};

use crate::KatanaRunner;

impl KatanaRunner {
    pub fn accounts_data(&self) -> &[Account] {
        &self.accounts[1..] // The first one is used to deploy the contract
    }

    pub fn accounts(&self) -> Vec<SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>> {
        self.accounts_data().iter().map(|account| self.account_to_single_owned(account)).collect()
    }

    pub fn account(
        &self,
        index: usize,
    ) -> SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> {
        self.account_to_single_owned(&self.accounts[index])
    }

    fn account_to_single_owned(
        &self,
        account: &Account,
    ) -> SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> {
        let private_key = SigningKey::from_secret_scalar(account.private_key);
        let signer = LocalWallet::from_signing_key(private_key);

        debug_assert_eq!(Environment::default().chain_id, ChainId::parse("KATANA").unwrap());
        let chain_id = felt!("82743958523457");
        let provider = self.owned_provider();

        SingleOwnerAccount::new(provider, signer, account.address, chain_id, ExecutionEncoding::New)
    }
}
