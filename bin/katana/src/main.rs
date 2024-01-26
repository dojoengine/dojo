use std::io;
use std::sync::Arc;

use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use console::Style;
use katana_core::sequencer::KatanaSequencer;
use katana_primitives::contract::ContractAddress;
use katana_primitives::genesis::allocation::GenesisAccountAlloc;
use katana_primitives::genesis::constant::{
    DEFAULT_FEE_TOKEN_ADDRESS, DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH,
    DEFAULT_LEGACY_UDC_CLASS_HASH, DEFAULT_UDC_ADDRESS,
};
use katana_rpc::{spawn, NodeHandle};
use metrics::prometheus_exporter;
use tokio::signal::ctrl_c;
use tracing::info;

mod args;
mod utils;

use args::Commands::Completions;
use args::KatanaArgs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = KatanaArgs::parse();
    config.init_logging()?;

    if let Some(command) = config.command {
        match command {
            Completions { shell } => {
                print_completion(shell);
                return Ok(());
            }
        }
    }

    let server_config = config.server_config();
    let sequencer_config = config.sequencer_config();
    let starknet_config = config.starknet_config();

    let sequencer = Arc::new(KatanaSequencer::new(sequencer_config, starknet_config).await?);
    let NodeHandle { addr, handle, .. } = spawn(Arc::clone(&sequencer), server_config).await?;

    if !config.silent {
        let mut accounts = sequencer.backend().config.genesis.accounts().peekable();
        let account_class_hash = accounts.peek().unwrap().1.class_hash();

        if config.json_log {
            info!(
                "{}",
                serde_json::json!({
                    "accounts": accounts.map(|a| serde_json::json!(a)).collect::<Vec<_>>(),
                    "seed": format!("{}", config.starknet.seed),
                    "address": format!("{addr}"),
                })
            )
        } else {
            // let accounts = accounts.map(|a| format!("{a}")).collect::<Vec<_>>().join("\n");
            print_intro(
                accounts,
                config.starknet.seed.clone(),
                format!(
                    "🚀 JSON-RPC server started: {}",
                    Style::new().red().apply_to(format!("http://{addr}"))
                ),
                format!("{:#064x}", account_class_hash),
            );
        }
    }

    if let Some(listen_addr) = config.metrics {
        let prometheus_handle = prometheus_exporter::install_recorder("katana")?;

        info!(target: "katana::cli", addr = %listen_addr, "Starting metrics endpoint");
        prometheus_exporter::serve(
            listen_addr,
            prometheus_handle,
            metrics_process::Collector::default(),
        )
        .await?;
    }

    // Wait until Ctrl + C is pressed, then shutdown
    ctrl_c().await?;
    handle.stop()?;

    Ok(())
}

fn print_completion(shell: Shell) {
    let mut command = KatanaArgs::command();
    let name = command.get_name().to_string();
    generate(shell, &mut command, name, &mut io::stdout());
}

fn print_intro<'a>(
    accounts: impl Iterator<Item = (&'a ContractAddress, &'a GenesisAccountAlloc)>,
    seed: String,
    address: String,
    account_class_hash: String,
) {
    println!(
        "{}",
        Style::new().red().apply_to(
            r"


██╗  ██╗ █████╗ ████████╗ █████╗ ███╗   ██╗ █████╗
██║ ██╔╝██╔══██╗╚══██╔══╝██╔══██╗████╗  ██║██╔══██╗
█████╔╝ ███████║   ██║   ███████║██╔██╗ ██║███████║
██╔═██╗ ██╔══██║   ██║   ██╔══██║██║╚██╗██║██╔══██║
██║  ██╗██║  ██║   ██║   ██║  ██║██║ ╚████║██║  ██║
╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═══╝╚═╝  ╚═╝
"
        )
    );

    println!(
        r"
PREDEPLOYED CONTRACTS
==================

| Contract        | Fee Token
| Address         | {DEFAULT_FEE_TOKEN_ADDRESS}
| Class Hash      | {DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH:#064x}

| Contract        | Universal Deployer
| Address         | {DEFAULT_UDC_ADDRESS}
| Class Hash      | {DEFAULT_LEGACY_UDC_CLASS_HASH:#064x}

| Contract        | Account Contract
| Class Hash      | {account_class_hash}
    "
    );

    print_genesis_accounts(accounts);

    println!(
        r"
ACCOUNTS SEED
=============
{seed}
    "
    );

    println!("\n{address}\n\n");
}

fn print_genesis_accounts<'a, Accounts>(accounts: Accounts)
where
    Accounts: Iterator<Item = (&'a ContractAddress, &'a GenesisAccountAlloc)>,
{
    println!(
        r"
PREFUNDED ACCOUNTS
=================="
    );

    for (addr, account) in accounts {
        if let Some(pk) = account.private_key() {
            println!(
                r"
| Account address |  {addr}
| Private key     |  {pk:#x}
| Public key      |  {:#x}",
                account.public_key()
            )
        } else {
            println!(
                r"
| Account address |  {addr}
| Public key      |  {:#x}",
                account.public_key()
            )
        }
    }
}
