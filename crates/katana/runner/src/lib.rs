mod logs;
mod prefunded;
mod utils;

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use assert_fs::TempDir;
use katana_primitives::contract::ContractAddress;
use katana_primitives::genesis::allocation::{DevAllocationsGenerator, DevGenesisAccount};
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
    accounts: Vec<(ContractAddress, DevGenesisAccount)>,
    log_file_path: PathBuf,
    contract: Mutex<Option<FieldElement>>,
}

/// Configuration for the KatanaRunner.
#[derive(Debug)]
pub struct KatanaRunnerConfig {
    /// The name of the katana program to run.
    pub program_name: Option<String>,
    /// The name used in the log file suffix, the port number is used otherwise.
    pub run_name: Option<String>,
    /// The number of accounts to predeployed.
    pub n_accounts: u16,
    /// Whether to disable the fee.
    pub disable_fee: bool,
    /// The block time in milliseconds.
    pub block_time: Option<u64>,
    /// The port to run the katana runner on, if None, a random free port is chosen.
    pub port: Option<u16>,
    /// The path where to log info, if None, logs are stored in a temp dir.
    pub log_path: Option<PathBuf>,
    /// The messaging config file
    pub messaging: Option<String>,
}

impl Default for KatanaRunnerConfig {
    fn default() -> Self {
        Self {
            n_accounts: 2,
            disable_fee: false,
            block_time: None,
            port: None,
            program_name: None,
            run_name: None,
            log_path: None,
            messaging: None,
        }
    }
}

impl KatanaRunner {
    /// Creates a new KatanaRunner with default values.
    pub fn new() -> Result<Self> {
        Self::setup_and_start(KatanaRunnerConfig::default())
    }

    /// Creates a new KatanaRunner with the given configuration.
    pub fn new_with_config(config: KatanaRunnerConfig) -> Result<Self> {
        Self::setup_and_start(config)
    }

    /// Starts a new KatanaRunner with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration for the katana runner.
    fn setup_and_start(config: KatanaRunnerConfig) -> Result<Self> {
        let program = config.program_name.clone().unwrap_or_else(determine_default_program_path);

        let port = config.port.unwrap_or_else(find_free_port);
        let n_accounts = config.n_accounts;

        let mut command = Command::new(program);
        command
            .args(["-p", &port.to_string()])
            .args(["--json-log"])
            .args(["--max-connections", &format!("{}", 10000)])
            .args(["--accounts", &format!("{}", n_accounts)]);

        if let Some(block_time_ms) = config.block_time {
            command.args(["--block-time", &format!("{}", block_time_ms)]);
        }

        if config.disable_fee {
            command.args(["--disable-fee"]);
        }

        if let Some(messaging_file) = config.messaging {
            command.args(["--messaging", messaging_file.as_str()]);
        }

        let mut child =
            command.stdout(Stdio::piped()).spawn().context("failed to start subprocess")?;

        let stdout = child.stdout.take().context("failed to take subprocess stdout")?;

        let log_filename = PathBuf::from(format!(
            "katana-{}.log",
            config.run_name.clone().unwrap_or_else(|| port.to_string())
        ));

        let log_file_path = if let Some(log_path) = config.log_path {
            log_path
        } else {
            let log_dir = TempDir::new().unwrap();
            log_dir.join(log_filename)
        };

        let log_file_path_sent = log_file_path.clone();

        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            utils::wait_for_server_started_and_signal(&log_file_path_sent, stdout, sender);
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

        Ok(KatanaRunner { child, port, provider, accounts, log_file_path, contract })
    }

    pub fn log_file_path(&self) -> &PathBuf {
        &self.log_file_path
    }

    pub fn provider(&self) -> &JsonRpcClient<HttpTransport> {
        &self.provider
    }

    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    pub fn url(&self) -> Url {
        Url::parse(&self.endpoint()).context("Failed to parse url").unwrap()
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

/// Determines the default program path for the katana runner based on the KATANA_RUNNER_BIN
/// environment variable. If not set, try to to use katana from the PATH.
fn determine_default_program_path() -> String {
    if let Ok(bin) = std::env::var("KATANA_RUNNER_BIN") { bin } else { "katana".to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_default_program_path() {
        // Set the environment variable to test the first branch
        std::env::set_var("KATANA_RUNNER_BIN", "custom_katana_path");
        assert_eq!(determine_default_program_path(), "custom_katana_path");

        // Unset the environment variable to test the fallback branch
        std::env::remove_var("KATANA_RUNNER_BIN");
        assert_eq!(determine_default_program_path(), "katana");
    }
}
