use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::macros::felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::{LocalWallet, SigningKey};

use crate::KatanaRunner;

impl KatanaRunner {
    pub fn accounts_data(&self) -> &[katana_core::accounts::Account] {
        &self.accounts[1..] // The first one is used to deploy the contract
    }

    pub fn accounts(&self) -> Vec<SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>> {
        self.accounts_data().iter().enumerate().map(|(i, _)| self.account(i)).collect()
    }

    pub fn account(
        &self,
        index: usize,
    ) -> SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> {
        let account = &self.accounts[index];
        let private_key = SigningKey::from_secret_scalar(account.private_key);
        let signer = LocalWallet::from_signing_key(private_key);

        debug_assert_eq!(katana_core::backend::config::Environment::default().chain_id, "KATANA");
        let chain_id = felt!("82743958523457");
        let provider = self.owned_provider();

        SingleOwnerAccount::new(provider, signer, account.address, chain_id, ExecutionEncoding::New)
    }
}
