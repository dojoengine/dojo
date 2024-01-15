use std::io;
use std::sync::Arc;

use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use console::Style;
use katana_core::constants::{
    ERC20_CONTRACT_CLASS_HASH, FEE_TOKEN_ADDRESS, UDC_ADDRESS, UDC_CLASS_HASH,
};
use katana_core::sequencer::KatanaSequencer;
use katana_rpc::{spawn, NodeHandle};
use metrics::prometheus_exporter;
use tokio::signal::ctrl_c;
use tracing::info;

mod args;

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
        let mut accounts = sequencer.backend.accounts.iter().peekable();
        let account_class_hash = accounts.peek().unwrap().class_hash;

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
            let accounts = accounts.map(|a| format!("{a}")).collect::<Vec<_>>().join("\n");
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
        let prometheus_handle = prometheus_exporter::install_recorder()?;

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

fn print_intro(accounts: String, seed: String, address: String, account_class_hash: String) {
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
| Address         | {}
| Class Hash      | {:#064x}

| Contract        | Universal Deployer
| Address         | {}
| Class Hash      | {:#064x}

| Contract        | Account Contract
| Class Hash      | {}
    ",
        *FEE_TOKEN_ADDRESS,
        *ERC20_CONTRACT_CLASS_HASH,
        *UDC_ADDRESS,
        *UDC_CLASS_HASH,
        account_class_hash
    );

    println!(
        r"        
PREFUNDED ACCOUNTS
==================
{accounts}
    "
    );

    println!(
        r"
ACCOUNTS SEED
=============
{seed}
    "
    );

    println!("\n{address}\n\n");
}
