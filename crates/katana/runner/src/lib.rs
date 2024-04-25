mod logs;
mod prefunded;
mod utils;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use ethers::middleware::SignerMiddleware;
use ethers::providers::{Http, Middleware, Provider as EthProvider};
use ethers::signers::{LocalWallet as EthLocalWallet, Signer};
use hyper::http::request;
use hyper::{Client, Response, StatusCode};
use katana_primitives::contract::ContractAddress;
use katana_primitives::genesis::allocation::{DevAllocationsGenerator, DevGenesisAccount};
use katana_primitives::FieldElement;
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

    pub fn new_with_name(name: &str) -> Result<Self> {
        Self::new_with_port_and_filename(
            "katana",
            find_free_port(),
            format!("logs/katana-{}.log", name),
            2,
            false,
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
        )
    }

    pub fn new_with_port(port: u16) -> Result<Self> {
        Self::new_with_port_and_filename(
            "katana",
            port,
            format!("katana-logs/{}.log", port),
            2,
            false,
        )
    }

    fn new_with_port_and_filename(
        program: &str,
        port: u16,
        log_filename: String,
        n_accounts: u16,
        with_blocks: bool,
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

pub struct AnvilRunner {
    process: Child,
    endpoint: String,
}

impl AnvilRunner {
    pub async fn new() -> Result<Self> {
        let port = find_free_port();
        let mut command = Command::new("Anvil");
        command
            .args(["-p", &port.to_string()])
            .args(["--silent"])
            .args(["--disable-block-gas-limit"])
            .spawn()
            .expect("Error executing Anvil");

        let endpoint = format!("http://127.0.0.1:{port}");

        if !is_anvil_up(&endpoint).await {
            panic!("Error bringing up Anvil")
        }

        let process =
            command.stdout(Stdio::piped()).spawn().context("failed to start subprocess")?;
        return Ok(AnvilRunner { process, endpoint });
    }

    fn provider(&self) -> EthProvider<Http> {
        return EthProvider::<Http>::try_from(&self.endpoint)
            .expect("Error getting provider")
            .interval(Duration::from_millis(10u64));
    }

    pub async fn account(&self) -> SignerMiddleware<EthProvider<Http>, EthLocalWallet> {
        let provider = self.provider();
        let wallet: EthLocalWallet =
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".parse().unwrap();
        let chain_id: ethers::types::U256 =
            provider.get_chainid().await.expect("Error getting chain id");
        return SignerMiddleware::new(provider, wallet.with_chain_id(chain_id.as_u32()));
    }

    pub fn endpoint(self) -> String {
        self.endpoint.clone()
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

    Client::new().request(req).await
}

async fn is_anvil_up(endpoint: &String) -> bool {
    let mut retries = 0;
    let max_retries = 10;
    while retries < max_retries {
        if let Ok(anvil_block_rsp) = post_dummy_request(&endpoint).await {
            assert_eq!(anvil_block_rsp.status(), StatusCode::OK);
            return true;
        }
        retries += 1;
        tokio::time::sleep(time::Duration::from_millis(500)).await;
    }
    false
}
