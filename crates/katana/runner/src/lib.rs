mod logs;
mod prefunded;
mod utils;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::str::FromStr;
use std::sync::mpsc::{self};
use std::thread;
use std::time::Duration;

use alloy::network::{Ethereum, EthereumSigner};
use alloy::primitives::Address;
use alloy::providers::fillers::{
    ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller, SignerFiller,
};
use alloy::providers::{Identity, ProviderBuilder, RootProvider};
use alloy::signers::wallet::LocalWallet;
use alloy::transports::http::Http;
use anyhow::{Context, Result};
use hyper::http::request;
use hyper::{Client as HyperClient, Response, StatusCode};
use katana_primitives::contract::ContractAddress;
use katana_primitives::genesis::allocation::{DevAllocationsGenerator, DevGenesisAccount};
use katana_primitives::FieldElement;
use reqwest::Client;
pub use runner_macro::{katana_test, runner};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use tokio::sync::Mutex;
use tokio::time;
use url::Url;
use utils::find_free_port;

#[derive(Debug)]
pub struct KatanaRunner {
    child: Child,
    port: u16,
    provider: JsonRpcClient<HttpTransport>,
    accounts: Vec<(ContractAddress, DevGenesisAccount)>,
    log_filename: PathBuf,
    contract: Mutex<Option<FieldElement>>,
}

pub const BLOCK_TIME_IF_ENABLED: u64 = 3000;

impl KatanaRunner {
    pub fn new() -> Result<Self> {
        Self::new_with_port(find_free_port())
    }

    pub fn new_with_messaging(file_path: String) -> Result<Self> {
        let port = find_free_port();
        Self::new_with_port_and_filename(
            "katana",
            find_free_port(),
            format!("logs/katana-{}.log", port),
            2,
            false,
            file_path,
        )
    }

    pub fn new_with_name(name: &str) -> Result<Self> {
        Self::new_with_port_and_filename(
            "katana",
            find_free_port(),
            format!("logs/katana-{}.log", name),
            2,
            false,
            "".to_string(),
        )
    }

    pub fn new_with_args(
        program: &str,
        name: &str,
        n_accounts: u16,
        with_blocks: bool,
    ) -> Result<Self> {
        Self::new_with_port_and_filename(
            program,
            find_free_port(),
            format!("katana-logs/{}.log", name),
            n_accounts,
            with_blocks,
            "".to_string(),
        )
    }

    pub fn new_with_port(port: u16) -> Result<Self> {
        Self::new_with_port_and_filename(
            "katana",
            port,
            format!("katana-logs/{}.log", port),
            2,
            false,
            "".to_string(),
        )
    }

    fn new_with_port_and_filename(
        program: &str,
        port: u16,
        log_filename: String,
        n_accounts: u16,
        with_blocks: bool,
        messaging_file_config: String,
    ) -> Result<Self> {
        let mut command = Command::new(program);
        command
            .args(["-p", &port.to_string()])
            .args(["--json-log"])
            .args(["--max-connections", &format!("{}", 10000)])
            .args(["--accounts", &format!("{}", n_accounts)]);

        if with_blocks {
            command.args(["--block-time", &format!("{}", BLOCK_TIME_IF_ENABLED)]);
        }

        if !messaging_file_config.is_empty() {
            command.args(["--messaging", messaging_file_config.as_str()]);
        }

        let mut child =
            command.stdout(Stdio::piped()).spawn().context("failed to start subprocess")?;

        let stdout = child.stdout.take().context("failed to take subprocess stdout")?;

        let log_filename_sent = PathBuf::from(log_filename);
        let log_filename = log_filename_sent.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            utils::wait_for_server_started_and_signal(&log_filename_sent, stdout, sender);
        });

        receiver
            .recv_timeout(Duration::from_secs(5))
            .context("timeout waiting for server to start")?;

        let url =
            Url::parse(&format!("http://127.0.0.1:{}/", port)).context("Failed to parse url")?;
        let provider = JsonRpcClient::new(HttpTransport::new(url));

        let mut seed = [0; 32];
        seed[0] = 48;
        let accounts = DevAllocationsGenerator::new(n_accounts)
            .with_seed(seed)
            .generate()
            .into_iter()
            .collect();
        let contract = Mutex::new(Option::None);

        Ok(KatanaRunner { child, port, provider, accounts, log_filename, contract })
    }

    pub fn provider(&self) -> &JsonRpcClient<HttpTransport> {
        &self.provider
    }

    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    pub fn owned_provider(&self) -> JsonRpcClient<HttpTransport> {
        let url = Url::parse(&self.endpoint()).context("Failed to parse url").unwrap();
        JsonRpcClient::new(HttpTransport::new(url))
    }

    // A constract needs to be deployed only once for each instance
    // In proptest runner is static but deployment would happen for each test, unless it is
    // persisted here.
    pub async fn set_contract(&self, contract_address: FieldElement) {
        let mut lock = self.contract.lock().await;
        *lock = Some(contract_address);
    }

    pub async fn contract(&self) -> Option<FieldElement> {
        *self.contract.lock().await
    }
}

impl Drop for KatanaRunner {
    fn drop(&mut self) {
        if let Err(e) = self.child.kill() {
            eprintln!("Failed to kill katana subprocess: {}", e);
        }
        if let Err(e) = self.child.wait() {
            eprintln!("Failed to wait for katana subprocess: {}", e);
        }
    }
}

type Provider = FillProvider<
    JoinFill<
        JoinFill<JoinFill<JoinFill<Identity, GasFiller>, NonceFiller>, ChainIdFiller>,
        SignerFiller<EthereumSigner>,
    >,
    RootProvider<Http<Client>>,
    Http<Client>,
    Ethereum,
>;

#[derive(Debug)]
pub struct AnvilRunner {
    process: Child,
    provider: Provider,
    pub endpoint: String,
    address: Address,
    secret_key: String,
}

impl AnvilRunner {
    pub async fn new() -> Result<Self> {
        let port = find_free_port();

        let process = Command::new("anvil")
            .arg("--port")
            .arg(&port.to_string())
            .arg("--silent")
            .arg("--disable-block-gas-limit")
            .spawn()
            .expect("Could not start background Anvil");

        let endpoint = format!("http://127.0.0.1:{port}");

        if !is_anvil_up(&endpoint).await {
            panic!("Error bringing up Anvil")
        }
        let secret_key =
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string();
        let wallet: LocalWallet = secret_key.parse().unwrap();

        let address = wallet.address();

        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .signer(EthereumSigner::from(wallet))
            .on_http(Url::from_str(&endpoint).unwrap());
        Ok(AnvilRunner { process, endpoint, provider, address, secret_key })
    }

    pub fn provider(&self) -> &Provider {
        &self.provider
    }

    pub fn endpoint(&self) -> String {
        self.endpoint.clone()
    }

    pub fn address(&self) -> Address {
        self.address
    }

    pub fn secret_key(&self) -> String {
        self.secret_key.clone()
    }
}

impl Drop for AnvilRunner {
    fn drop(&mut self) {
        self.process.kill().expect("Cannot kill process");
    }
}

async fn post_dummy_request(url: &String) -> Result<Response<hyper::Body>, hyper::Error> {
    let req = request::Request::post(url)
        .header("content-type", "application/json")
        .body(hyper::Body::from(
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "eth_blockNumberfuiorhgorueh",
                "params": [],
                "id": "1"
            })
            .to_string(),
        ))
        .unwrap();

    HyperClient::new().request(req).await
}

async fn is_anvil_up(endpoint: &String) -> bool {
    let mut retries = 0;
    let max_retries = 10;
    while retries < max_retries {
        if let Ok(anvil_block_rsp) = post_dummy_request(endpoint).await {
            assert_eq!(anvil_block_rsp.status(), StatusCode::OK);
            return true;
        }
        retries += 1;
        tokio::time::sleep(time::Duration::from_millis(500)).await;
    }
    false
}
