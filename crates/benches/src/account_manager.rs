use std::sync::Arc;

use futures::future::join_all;
use katana_core::accounts::DevAccountGenerator;
use starknet::{
    accounts::{Account, ConnectedAccount, ExecutionEncoding, SingleOwnerAccount},
    core::types::FieldElement,
    providers::{jsonrpc::HttpTransport, JsonRpcClient},
    signers::{LocalWallet, SigningKey},
};
use tokio::sync::{Mutex, OnceCell};

use crate::{
    helpers::{chain_id, provider},
    ACCOUNT_ADDRESS,
};

pub async fn account_manager() -> Arc<AccountManager> {
    static CHAIN_ID: OnceCell<Arc<AccountManager>> = OnceCell::const_new();

    CHAIN_ID
        .get_or_init(|| async {
            let mut accounts = AccountManager::generate().await;

            let shared = accounts.remove(0); // remove the first account (it's the default account)
            debug_assert_eq!(
                shared.0.address(),
                FieldElement::from_hex_be(ACCOUNT_ADDRESS).unwrap()
            );

            Arc::new(AccountManager { head: Arc::default(), accounts })
        })
        .await
        .clone()
}

pub type OwnerAccount = SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>;
pub type AccountWithNonce = (Arc<OwnerAccount>, Mutex<FieldElement>);

pub struct AccountManager {
    accounts: Vec<AccountWithNonce>,
    head: Arc<Mutex<usize>>,
}

impl AccountManager {
    async fn generate() -> Vec<AccountWithNonce> {
        let mut seed = [0; 32];
        seed[0] = 48;

        let accounts = DevAccountGenerator::new(1000).with_seed(seed).generate();

        let chain_id: FieldElement = chain_id().await;

        let account_futures = accounts
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

                async move {
                    let nonce = account.get_nonce().await.expect("Failed to get nonce");
                    (Arc::new(account), Mutex::new(nonce))
                }
            })
            .collect::<Vec<_>>();

        join_all(account_futures).await
    }

    pub async fn next(&self) -> (Arc<OwnerAccount>, FieldElement) {
        let mut head = self.head.lock().await;
        *head = (*head + 1) % self.accounts.len();

        let mut nonce_lock = self.accounts[*head].1.lock().await;
        let nonce = nonce_lock.clone();
        *nonce_lock += FieldElement::ONE;

        (self.accounts[*head].0.clone(), nonce)
    }
}
