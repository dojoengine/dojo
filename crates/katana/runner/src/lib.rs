#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod utils;

use std::path::PathBuf;
use std::thread;

use anyhow::{Context, Result};
use assert_fs::TempDir;
use katana_node_bindings::{Katana, KatanaInstance};
pub use katana_runner_macro::test;
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::{BlockId, BlockTag, Felt};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::LocalWallet;
use tokio::sync::Mutex;
use url::Url;
use utils::find_free_port;

#[allow(dead_code)]
#[derive(Debug)]
pub struct RunnerCtx(KatanaRunner);

impl RunnerCtx {
    pub fn new(runner: KatanaRunner) -> Self {
        Self(runner)
    }
}

impl core::ops::Deref for RunnerCtx {
    type Target = KatanaRunner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub struct KatanaRunner {
    instance: KatanaInstance,
    provider: JsonRpcClient<HttpTransport>,
    log_file_path: PathBuf,
    contract: Mutex<Option<Felt>>,
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
    /// The path to the database dir.
    pub db_dir: Option<PathBuf>,
    /// Whether to run the katana runner with the `dev` rpc endpoints.
    pub dev: bool,
    /// The chain id to use.
    pub chain_id: Option<Felt>,
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
            db_dir: None,
            dev: false,
            chain_id: None,
        }
    }
}

impl KatanaRunnerConfig {
    pub fn with_db_dir(mut self, db_dir: &str) -> Self {
        self.db_dir = Some(PathBuf::from(db_dir));
        self
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
        let program = config.program_name.unwrap_or_else(determine_default_program_path);
        let port = config.port.unwrap_or_else(find_free_port);
        let n_accounts = config.n_accounts;

        let mut builder = Katana::new()
            .path(program)
            .port(port)
            .accounts(n_accounts)
            .json_log(true)
            .rpc_max_connections(10000)
            .dev(config.dev)
            .fee(!config.disable_fee);

        if let Some(id) = config.chain_id {
            builder = builder.chain_id(id);
        }

        if let Some(block_time_ms) = config.block_time {
            builder = builder.block_time(block_time_ms);
        }

        if let Some(messaging_file) = config.messaging {
            builder = builder.messaging(messaging_file);
        }

        if let Some(path) = config.db_dir {
            builder = builder.db_dir(path);
        }

        builder = builder.dev(config.dev);

        // start the katana instance
        let mut instance = builder.spawn();

        let stdout =
            instance.child_mut().stdout.take().context("failed to take subprocess stdout")?;

        let log_filename = PathBuf::from(format!(
            "katana-{}.log",
            config.run_name.unwrap_or_else(|| port.to_string())
        ));

        let log_file_path = if let Some(log_path) = config.log_path {
            log_path
        } else {
            let log_dir = TempDir::new().unwrap();
            log_dir.join(log_filename)
        };

        let log_file_path_sent = log_file_path.clone();
        thread::spawn(move || {
            utils::listen_to_stdout(&log_file_path_sent, stdout);
        });

        let provider = JsonRpcClient::new(HttpTransport::new(instance.endpoint_url()));
        let contract = Mutex::new(Option::None);

        Ok(KatanaRunner { instance, provider, log_file_path, contract })
    }

    pub fn log_file_path(&self) -> &PathBuf {
        &self.log_file_path
    }

    pub fn provider(&self) -> &JsonRpcClient<HttpTransport> {
        &self.provider
    }

    pub fn endpoint(&self) -> String {
        self.instance.endpoint()
    }

    pub fn url(&self) -> Url {
        self.instance.endpoint_url()
    }

    pub fn owned_provider(&self) -> JsonRpcClient<HttpTransport> {
        JsonRpcClient::new(HttpTransport::new(self.url()))
    }

    // A contract needs to be deployed only once for each instance
    // In proptest runner is static but deployment would happen for each test, unless it is
    // persisted here.
    pub async fn set_contract(&self, contract_address: Felt) {
        let mut lock = self.contract.lock().await;
        *lock = Some(contract_address);
    }

    pub async fn contract(&self) -> Option<Felt> {
        *self.contract.lock().await
    }

    pub fn accounts_data(&self) -> &[katana_node_bindings::Account] {
        self.instance.accounts()
    }

    pub fn accounts(&self) -> Vec<SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>> {
        self.accounts_data().iter().map(|account| self.account_to_single_owned(account)).collect()
    }

    pub fn account_data(&self, index: usize) -> &katana_node_bindings::Account {
        &self.accounts_data()[index]
    }

    pub fn account(
        &self,
        index: usize,
    ) -> SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> {
        self.account_to_single_owned(&self.accounts_data()[index])
    }

    fn account_to_single_owned(
        &self,
        account: &katana_node_bindings::Account,
    ) -> SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> {
        let signer = if let Some(private_key) = &account.private_key {
            LocalWallet::from(private_key.clone())
        } else {
            panic!("Account does not have a private key")
        };

        let chain_id = self.instance.chain_id();
        let provider = self.owned_provider();

        let mut account = SingleOwnerAccount::new(
            provider,
            signer,
            account.address,
            chain_id,
            ExecutionEncoding::New,
        );

        account.set_block_id(BlockId::Tag(BlockTag::Pending));

        account
    }
}

/// Determines the default program path for the katana runner based on the KATANA_RUNNER_BIN
/// environment variable. If not set, try to to use katana from the PATH.
fn determine_default_program_path() -> String {
    if let Ok(bin) = std::env::var("KATANA_RUNNER_BIN") { bin } else { "katana".to_string() }
}

#[cfg(test)]
mod tests {
    use crate::determine_default_program_path;

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
