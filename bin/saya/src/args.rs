//! Saya binary options.
use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use tracing::Subscriber;
use tracing_subscriber::{fmt, EnvFilter};
use url::Url;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct SayaArgs {
    /// Specify the Katana URL to fetch data from.
    #[arg(long)]
    #[arg(value_name = "KATANA URL")]
    #[arg(help = "The Katana RPC URL to fetch data from.")]
    pub rpc_url: Option<Url>,

    /// Enable JSON logging.
    #[arg(long)]
    #[arg(help = "Output logs in JSON format.")]
    pub json_log: bool,

    /// Specify a block to start fetching data from.
    #[arg(short, long, default_value = "0")]
    pub start_block: u64,
}

impl SayaArgs {
    pub fn init_logging(&self) -> Result<(), Box<dyn std::error::Error>> {
        const DEFAULT_LOG_FILTER: &str = "info,saya_core=trace,hyper=off";

        let builder = fmt::Subscriber::builder().with_env_filter(
            EnvFilter::try_from_default_env().or(EnvFilter::try_new(DEFAULT_LOG_FILTER))?,
        );

        let subscriber: Box<dyn Subscriber + Send + Sync> = if self.json_log {
            Box::new(builder.json().finish())
        } else {
            Box::new(builder.finish())
        };

        Ok(tracing::subscriber::set_global_default(subscriber)?)
    }
}
