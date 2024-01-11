mod deployer;
mod utils;

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use katana_core::accounts::DevAccountGenerator;
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::macros::felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::{LocalWallet, SigningKey};
use url::Url;

pub use runner_macro::katana_test;
use utils::find_free_port;

#[derive(Debug)]
pub struct KatanaRunner {
    child: Child,
    port: u16,
    provider: JsonRpcClient<HttpTransport>,
    accounts: Vec<katana_core::accounts::Account>,
}

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
        )
    }

    pub fn new_with_args(program: &str, name: &str, n_accounts: u16) -> Result<Self> {
        Self::new_with_port_and_filename(
            program,
            find_free_port(),
            format!("katana-logs/katana-{}.log", name),
            n_accounts,
        )
    }

    pub fn new_with_port(port: u16) -> Result<Self> {
        Self::new_with_port_and_filename(
            "katana",
            port,
            format!("katana-logs/katana-{}.log", port),
            2,
        )
    }
    fn new_with_port_and_filename(
        program: &str,
        port: u16,
        log_filename: String,
        n_accounts: u16,
    ) -> Result<Self> {
        let mut child = Command::new(program)
            .args(["-p", &port.to_string()])
            .args(["--json-log"])
            .args(["--max-connections", &format!("{}", 10000)])
            .args(["--accounts", &format!("{}", n_accounts)])
            .stdout(Stdio::piped())
            .spawn()
            .context("failed to start subprocess")?;

        let stdout = child.stdout.take().context("failed to take subprocess stdout")?;

        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            utils::wait_for_server_started_and_signal(Path::new(&log_filename), stdout, sender);
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

        Ok(KatanaRunner { child, port, provider, accounts })
    }

    pub fn provider(&self) -> &JsonRpcClient<HttpTransport> {
        &self.provider
    }

    pub fn owned_provider(&self) -> JsonRpcClient<HttpTransport> {
        let url = Url::parse(&format!("http://127.0.0.1:{}/", self.port))
            .context("Failed to parse url")
            .unwrap();
        JsonRpcClient::new(HttpTransport::new(url))
    }

    pub fn accounts(&self) -> &[katana_core::accounts::Account] {
        &self.accounts
    }

    pub fn account(
        &self,
        index: usize,
    ) -> SingleOwnerAccount<&JsonRpcClient<HttpTransport>, LocalWallet> {
        let account = &self.accounts[index];
        let private_key = SigningKey::from_secret_scalar(account.private_key);
        let signer = LocalWallet::from_signing_key(private_key);

        debug_assert_eq!(katana_core::backend::config::Environment::default().chain_id, "KATANA");
        let chain_id = felt!("82743958523457");
        let provider = self.provider();

        SingleOwnerAccount::new(
            provider,
            signer,
            account.address,
            chain_id,
            ExecutionEncoding::Legacy,
        )
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
