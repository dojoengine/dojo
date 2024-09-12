use std::str::FromStr;

use katana_runner::KatanaRunner;
use serde::{Deserialize, Serialize};
use starknet_crypto::Felt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub address: Felt,
    pub private_key: Felt,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum L2 {
    #[serde(skip)]
    Katana(KatanaRunner),
    Real {
        endpoint: String,
        account: Account,
    },
}

impl L2 {
    pub fn local_katana(port: &str) -> Self {
        L2::Real {
            endpoint: format!("http://0.0.0.0:{port}/"),
            account: Account {
                address: Felt::from_str(
                    "0xb3ff441a68610b30fd5e2abbf3a1548eb6ba6f3559f2862bf2dc757e5828ca",
                )
                .unwrap(),
                private_key: Felt::from_str(
                    "0x2bbf4f9fd0bbb2e60b0316c1fe0b76cf7a4d0198bd493ced9b8df2a3a24d68a",
                )
                .unwrap(),
            },
        }
    }
    pub fn endpoint(&self) -> String {
        match self {
            L2::Katana(k) => k.endpoint(),
            L2::Real { endpoint, .. } => endpoint.clone(),
        }
    }
    pub fn account(&self) -> Account {
        match self {
            L2::Katana(k) => {
                let a = k.account_data(0);
                Account {
                    address: a.address.clone(),
                    private_key: a.private_key.clone().unwrap().secret_scalar(),
                }
            }
            L2::Real { account, .. } => account.clone(),
        }
    }
}
