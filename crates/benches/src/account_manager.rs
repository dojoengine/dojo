use std::sync::Arc;

use katana_core::accounts::DevAccountGenerator;
use starknet::{
    accounts::{Account, ExecutionEncoding, SingleOwnerAccount},
    providers::{jsonrpc::HttpTransport, JsonRpcClient},
    signers::{LocalWallet, SigningKey},
};
use tokio::sync::OnceCell;

use crate::helpers::{chain_id, provider};

pub async fn account_manager() -> Arc<AccountManager> {
    static CHAIN_ID: OnceCell<Arc<AccountManager>> = OnceCell::const_new();

    CHAIN_ID
        .get_or_init(|| async {
            let mut accounts = AccountManager::generate().await;

            let shared = accounts.remove(0); // remove the first account (it's the default account)
            dbg!(shared.address());

            Arc::new(AccountManager { shared, accounts })
        })
        .await
        .clone()
}

pub type OwnerAccount = SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>;

#[derive(Clone)]
pub struct AccountManager {
    shared: Arc<OwnerAccount>,
    accounts: Vec<Arc<OwnerAccount>>,
}

impl AccountManager {
    async fn generate() -> Vec<Arc<OwnerAccount>> {
        let accounts = DevAccountGenerator::new(255).with_seed([0; 32]).generate();
        let chain_id = chain_id().await;

        accounts
            .into_iter()
            .map(|account| {
                let private_key = SigningKey::from_secret_scalar(account.private_key);
                let signer = LocalWallet::from_signing_key(private_key);
                let account = SingleOwnerAccount::new(
                    provider(),
                    signer,
                    account.address,
                    chain_id,
                    ExecutionEncoding::Legacy,
                );
                Arc::new(account)
            })
            .collect()
    }

    pub fn shared(&self) -> Arc<OwnerAccount> {
        self.shared.clone()
    }
}
