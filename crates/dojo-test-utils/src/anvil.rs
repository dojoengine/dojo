use std::time::Duration;

use ethers::middleware::SignerMiddleware;
use ethers::providers::{Http, Provider};
use ethers::signers::LocalWallet;
use ethers::utils::AnvilInstance;
use ethers_core::utils::Anvil;

pub struct TestAnvil {
    url: String,
    pub anvil: AnvilInstance,
}

impl TestAnvil {
    pub async fn start() -> Self {
        let port = 8545u16;
        let anvil = Anvil::new().port(port).spawn();

        let url = anvil.endpoint();
        TestAnvil { url, anvil }
    }

    pub fn provider(&self) -> Provider<Http> {
        return Provider::<Http>::try_from(&self.url)
            .expect("Error getting provider")
            .interval(Duration::from_millis(10u64));
    }

    pub fn account(&self) -> SignerMiddleware<Provider<Http>, LocalWallet> {
        SignerMiddleware::new(self.provider(), self.anvil.keys()[0].clone().into())
    }

    pub fn url(&self) -> String {
        self.url.clone()
    }

    pub fn stop(self) -> () {
        drop(self.anvil);
    }
}
