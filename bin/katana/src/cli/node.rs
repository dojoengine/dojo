//! Katana binary executable.
//!
//! ## Feature Flags
//!
//! - `jemalloc`: Uses [jemallocator](https://github.com/tikv/jemallocator) as the global allocator.
//!   This is **not recommended on Windows**. See [here](https://rust-lang.github.io/rfcs/1974-global-allocators.html#jemalloc)
//!   for more info.
//! - `jemalloc-prof`: Enables [jemallocator's](https://github.com/tikv/jemallocator) heap profiling
//!   and leak detection functionality. See [jemalloc's opt.prof](https://jemalloc.net/jemalloc.3.html#opt.prof)
//!   documentation for usage details. This is **not recommended on Windows**. See [here](https://rust-lang.github.io/rfcs/1974-global-allocators.html#jemalloc)
//!   for more info.

use anyhow::{Context, Result};
use console::Style;
use katana_cli::node::NodeArgs;
use katana_cli::utils::LogFormat;
use katana_primitives::chain_spec::ChainSpec;
use katana_primitives::class::ClassHash;
use katana_primitives::contract::ContractAddress;
use katana_primitives::genesis::allocation::GenesisAccountAlloc;
use katana_primitives::genesis::constant::{
    DEFAULT_LEGACY_ERC20_CLASS_HASH, DEFAULT_LEGACY_UDC_CLASS_HASH, DEFAULT_UDC_ADDRESS,
};
use tracing::{info, Subscriber};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt, EnvFilter};

pub(crate) const LOG_TARGET: &str = "katana::cli";

pub fn execute(args: &NodeArgs) -> Result<()> {
    init_logging(args)?;
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime")?
        .block_on(start_node(args))
}

async fn start_node(args: &NodeArgs) -> Result<()> {
    // Build the node
    let config = args.config()?;
    let node = katana_node::build(config).await.context("failed to build node")?;

    if !args.silent {
        print_intro(args, &node.backend.chain_spec);
    }

    // Launch the node
    let handle = node.launch().await.context("failed to launch node")?;

    // Wait until an OS signal (ie SIGINT, SIGTERM) is received or the node is shutdown.
    tokio::select! {
        _ = dojo_utils::signal::wait_signals() => {
            // Gracefully shutdown the node before exiting
            handle.stop().await?;
        },

        _ = handle.stopped() => { }
    }

    info!("Shutting down.");

    Ok(())
}

fn init_logging(args: &NodeArgs) -> Result<()> {
    const DEFAULT_LOG_FILTER: &str = "info,tasks=debug,executor=trace,forking::backend=trace,\
                                      blockifier=off,jsonrpsee_server=off,hyper=off,\
                                      messaging=debug,node=error";

    let filter = if args.development.dev {
        &format!("{DEFAULT_LOG_FILTER},server=debug")
    } else {
        DEFAULT_LOG_FILTER
    };

    LogTracer::init()?;

    // If the user has set the `RUST_LOG` environment variable, then we prioritize it.
    // Otherwise, we use the default log filter.
    // TODO: change env var to `KATANA_LOG`.
    let filter = EnvFilter::try_from_default_env().or(EnvFilter::try_new(filter))?;
    let builder = fmt::Subscriber::builder().with_env_filter(filter);

    let subscriber: Box<dyn Subscriber + Send + Sync> = match args.logging.log_format {
        LogFormat::Full => Box::new(builder.finish()),
        LogFormat::Json => Box::new(builder.json().finish()),
    };

    Ok(tracing::subscriber::set_global_default(subscriber)?)
}

fn print_intro(args: &NodeArgs, chain: &ChainSpec) {
    let mut accounts = chain.genesis.accounts().peekable();
    let account_class_hash = accounts.peek().map(|e| e.1.class_hash());
    let seed = &args.development.seed;

    if args.logging.log_format == LogFormat::Json {
        info!(
            target: LOG_TARGET,
            "{}",
            serde_json::json!({
                "accounts": accounts.map(|a| serde_json::json!(a)).collect::<Vec<_>>(),
                "seed": format!("{}", seed),
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

        print_genesis_contracts(chain, account_class_hash);
        print_genesis_accounts(accounts);

        println!(
            r"

ACCOUNTS SEED
=============
{seed}
    "
        );
    }
}

fn print_genesis_contracts(chain: &ChainSpec, account_class_hash: Option<ClassHash>) {
    println!(
        r"
PREDEPLOYED CONTRACTS
==================

| Contract        | ETH Fee Token
| Address         | {}
| Class Hash      | {:#064x}

| Contract        | STRK Fee Token
| Address         | {}
| Class Hash      | {:#064x}",
        chain.fee_contracts.eth,
        DEFAULT_LEGACY_ERC20_CLASS_HASH,
        chain.fee_contracts.strk,
        DEFAULT_LEGACY_ERC20_CLASS_HASH
    );

    println!(
        r"
| Contract        | Universal Deployer
| Address         | {}
| Class Hash      | {:#064x}",
        DEFAULT_UDC_ADDRESS, DEFAULT_LEGACY_UDC_CLASS_HASH
    );

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
