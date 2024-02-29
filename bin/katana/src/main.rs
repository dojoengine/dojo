use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use console::Style;
use katana_core::constants::MAX_RECURSION_DEPTH;
use katana_core::env::get_default_vm_resource_fee_cost;
use katana_core::sequencer::KatanaSequencer;
use katana_executor::SimulationFlag;
use katana_primitives::class::ClassHash;
use katana_primitives::contract::ContractAddress;
use katana_primitives::env::{CfgEnv, FeeTokenAddressses};
use katana_primitives::genesis::allocation::GenesisAccountAlloc;
use katana_primitives::genesis::Genesis;
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
    let args = KatanaArgs::parse();
    args.init_logging()?;

    if let Some(command) = args.command {
        match command {
            Completions { shell } => {
                print_completion(shell);
                return Ok(());
            }
        }
    }

    let server_config = args.server_config();
    let sequencer_config = args.sequencer_config();
    let starknet_config = args.starknet_config();

    let cfg_env = CfgEnv {
        chain_id: starknet_config.env.chain_id,
        vm_resource_fee_cost: get_default_vm_resource_fee_cost(),
        invoke_tx_max_n_steps: starknet_config.env.invoke_max_steps,
        validate_max_n_steps: starknet_config.env.validate_max_steps,
        max_recursion_depth: MAX_RECURSION_DEPTH,
        fee_token_addresses: FeeTokenAddressses {
            eth: starknet_config.genesis.fee_token.address,
            strk: Default::default(),
        },
    };

    let simulation_flags = SimulationFlag {
        skip_validate: starknet_config.disable_validate,
        skip_fee_transfer: starknet_config.disable_fee,
        ..Default::default()
    };

    cfg_if::cfg_if! {
        if #[cfg(all(feature = "blockifier", feature = "sir"))] {
            compile_error!("Cannot enable both `blockifier` and `sir` features at the same time");
        } else if #[cfg(feature = "blockifier")] {
            use katana_executor::implementation::blockifier::BlockifierFactory;
            let executor_factory = BlockifierFactory::new(cfg_env, simulation_flags);
        } else if #[cfg(feature = "sir")] {
            use katana_executor::implementation::sir::NativeExecutorFactory;
            let executor_factory = NativeExecutorFactory::new(cfg_env, simulation_flags);
        } else {
            compile_error!("At least one of the following features must be enabled: blockifier, sir");
        }
    }

    let sequencer =
        Arc::new(KatanaSequencer::new(executor_factory, sequencer_config, starknet_config).await?);
    let NodeHandle { addr, handle, .. } = spawn(Arc::clone(&sequencer), server_config).await?;

    if !args.silent {
        let genesis = &sequencer.backend().config.genesis;
        print_intro(&args, genesis, addr);
    }

    if let Some(listen_addr) = args.metrics {
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

fn print_intro(args: &KatanaArgs, genesis: &Genesis, address: SocketAddr) {
    let mut accounts = genesis.accounts().peekable();
    let account_class_hash = accounts.peek().map(|e| e.1.class_hash());
    let seed = &args.starknet.seed;

    if args.json_log {
        info!(
            "{}",
            serde_json::json!({
                "accounts": accounts.map(|a| serde_json::json!(a)).collect::<Vec<_>>(),
                "seed": format!("{}", seed),
                "address": format!("{address}"),
            })
        )
    } else {
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

        print_genesis_contracts(genesis, account_class_hash);
        print_genesis_accounts(accounts);

        println!(
            r"

ACCOUNTS SEED
=============
{seed}
    "
        );

        let addr = format!(
            "🚀 JSON-RPC server started: {}",
            Style::new().red().apply_to(format!("http://{address}"))
        );

        println!("\n{addr}\n\n",);
    }
}

fn print_genesis_contracts(genesis: &Genesis, account_class_hash: Option<ClassHash>) {
    println!(
        r"
PREDEPLOYED CONTRACTS
==================

| Contract        | Fee Token
| Address         | {}
| Class Hash      | {:#064x}",
        genesis.fee_token.address, genesis.fee_token.class_hash,
    );

    if let Some(ref udc) = genesis.universal_deployer {
        println!(
            r"
| Contract        | Universal Deployer
| Address         | {}
| Class Hash      | {:#064x}",
            udc.address, udc.class_hash
        )
    }

    if let Some(hash) = account_class_hash {
        println!(
            r"
| Contract        | Account Contract
| Class Hash      | {hash:#064x}"
        )
    }
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
