mod deployer;
mod utils;

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use katana_core::accounts::DevAccountGenerator;
pub use runner_macro::{katana_test, runner};
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::macros::felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::{LocalWallet, SigningKey};
use tokio::time::sleep;
use url::Url;
use utils::find_free_port;

#[derive(Debug)]
pub struct KatanaRunner {
    child: Child,
    port: u16,
    provider: JsonRpcClient<HttpTransport>,
    accounts: Vec<katana_core::accounts::Account>,
    log_filename: PathBuf,
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

        Ok(KatanaRunner { child, port, provider, accounts, log_filename })
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

    pub fn blocks(&self) -> Vec<String> {
        BufReader::new(File::open(&self.log_filename).unwrap())
            .lines()
            .filter_map(|line| {
                let line = line.unwrap();
                if line.contains("⛏️ Block") {
                    Some(line)
                } else {
                    None
                }
            })
            .collect()
    }

    pub async fn blocks_until_empty(&self) -> Vec<String> {
        let mut blocks = self.blocks();
        loop {
            if let Some(block) = blocks.last() {
                println!("{}", block);
                if block.contains("mined with 0 transactions") {
                    break;
                }
            }

            let len_at_call = blocks.len();
            while len_at_call == blocks.len() {
                sleep(Duration::from_millis(BLOCK_TIME_IF_ENABLED)).await;
                blocks = self.blocks();
            }
        }
        blocks
    }

    pub async fn block_sizes(&self) -> Vec<u32> {
        self.blocks_until_empty()
            .await
            .iter()
            .map(|block| {
                let limit =
                    block.find(" transactions").expect("Failed to find transactions in block");
                let number = block[..limit].split(" ").last().unwrap();
                number.parse::<u32>().expect("Failed to parse block number")
            })
            .collect()
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
