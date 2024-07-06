use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::{BlockId, BlockTag};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::LocalWallet;

use crate::KatanaRunner;

impl KatanaRunner {
    pub fn accounts_data(&self) -> &[katana_node_bindings::Account] {
        self.instance.accounts()
    }

    pub fn accounts(&self) -> Vec<SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>> {
        self.accounts_data().iter().map(|account| self.account_to_single_owned(account)).collect()
    }

    pub fn account_data(&self, index: usize) -> &katana_node_bindings::Account {
        &self.accounts_data()[index]
    }

    pub fn account(
        &self,
        index: usize,
    ) -> SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> {
        self.account_to_single_owned(&self.accounts_data()[index])
    }

    fn account_to_single_owned(
        &self,
        account: &katana_node_bindings::Account,
    ) -> SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> {
        let signer = if let Some(private_key) = &account.private_key {
            LocalWallet::from(private_key.clone())
        } else {
            panic!("Account does not have a private key")
        };

        let chain_id = self.instance.chain_id();
        let provider = self.owned_provider();

        let mut account = SingleOwnerAccount::new(
            provider,
            signer,
            account.address,
            chain_id,
            ExecutionEncoding::New,
        );

        account.set_block_id(BlockId::Tag(BlockTag::Pending));

        account
    }
}
