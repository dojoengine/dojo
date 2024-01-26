mod logs;
mod prefunded;
mod utils;

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use katana_core::accounts::DevAccountGenerator;
use katana_primitives::FieldElement;
pub use runner_macro::{katana_test, runner};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use tokio::sync::Mutex;
use url::Url;
use utils::find_free_port;

#[derive(Debug)]
pub struct KatanaRunner {
    child: Child,
    port: u16,
    provider: JsonRpcClient<HttpTransport>,
    accounts: Vec<katana_core::accounts::Account>,
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
        let accounts = DevAccountGenerator::new(n_accounts).with_seed(seed).generate();
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
    // In proptest runner is static but deployment would happen for each test, unless it is persisted here.
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
