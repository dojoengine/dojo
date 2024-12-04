use std::net::IpAddr;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser};
use katana_node::config::db::DbConfig;
use katana_node::config::metrics::{DEFAULT_METRICS_ADDR, DEFAULT_METRICS_PORT};
use katana_node::full::{Config, Node};

#[derive(Debug, Args, Clone, PartialEq)]
#[command(next_help_heading = "Metrics options")]
pub struct MetricsOptions {
    /// Enable metrics.
    ///
    /// For now, metrics will still be collected even if this flag is not set. This only
    /// controls whether the metrics server is started or not.
    #[arg(long)]
    pub metrics: bool,

    /// The metrics will be served at the given address.
    #[arg(requires = "metrics")]
    #[arg(long = "metrics.addr", value_name = "ADDRESS")]
    #[arg(default_value_t = DEFAULT_METRICS_ADDR)]
    pub metrics_addr: IpAddr,

    /// The metrics will be served at the given port.
    #[arg(requires = "metrics")]
    #[arg(long = "metrics.port", value_name = "PORT")]
    #[arg(default_value_t = DEFAULT_METRICS_PORT)]
    pub metrics_port: u16,
}

#[derive(Parser)]
pub struct Cli {
    /// Directory path of the database to initialize from.
    ///
    /// The path must either be an empty directory or a directory which already contains a
    /// previously initialized Katana database.
    #[arg(long)]
    #[arg(value_name = "PATH")]
    db_dir: PathBuf,

    #[arg(long)]
    #[arg(value_name = "API_KEY")]
    gateway_api_key: Option<String>,

    #[command(flatten)]
    metrics: MetricsOptions,
}

fn init_logging() -> Result<()> {
    use tracing::subscriber::set_global_default;
    use tracing_log::LogTracer;
    use tracing_subscriber::{fmt, EnvFilter};

    const DEFAULT_LOG_FILTER: &str = "pipeline=debug,stage=debug,info,tasks=debug,executor=trace,\
                                      forking::backend=trace,blockifier=off,jsonrpsee_server=off,\
                                      hyper=off,messaging=debug";

    LogTracer::init()?;

    let filter = EnvFilter::try_from_default_env().or(EnvFilter::try_new(DEFAULT_LOG_FILTER))?;
    let subscriber = fmt::Subscriber::builder().with_env_filter(filter).finish();
    set_global_default(subscriber)?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logging()?;

    let cli = Cli::parse();

    let config = Config {
        metrics: None,
        gateway_api_key: cli.gateway_api_key,
        db: DbConfig { dir: Some(cli.db_dir) },
    };

    let node = Node::build(config)?.launch()?;

    tokio::select! {
        _ = dojo_utils::signal::wait_signals() => {
            // Gracefully shutdown the node before exiting
            node.stop().await?;
        },

        _ = node.stopped() => { }
    }

    Ok(())
}
