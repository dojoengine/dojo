//! Saya binary options.
use clap::Parser;
use saya_core::data_availability::celestia::CelestiaConfig;
use saya_core::data_availability::DataAvailabilityConfig;
use saya_core::SayaConfig;
use tracing::Subscriber;
use tracing_subscriber::{fmt, EnvFilter};
use url::Url;

use crate::args::data_availability::{DataAvailabilityChain, DataAvailabilityOptions};

mod data_availability;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct SayaArgs {
    /// Specify the Katana URL to fetch data from.
    #[arg(long)]
    #[arg(value_name = "KATANA URL")]
    #[arg(help = "The Katana RPC URL to fetch data from.")]
    pub rpc_url: Url,

    /// Enable JSON logging.
    #[arg(long)]
    #[arg(help = "Output logs in JSON format.")]
    pub json_log: bool,

    /// Specify a block to start fetching data from.
    #[arg(short, long, default_value = "0")]
    pub start_block: u64,

    #[command(flatten)]
    #[command(next_help_heading = "Data availability options")]
    pub data_availability: DataAvailabilityOptions,
}

impl SayaArgs {
    pub fn init_logging(&self) -> Result<(), Box<dyn std::error::Error>> {
        const DEFAULT_LOG_FILTER: &str = "info,saya_core=trace";

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

impl TryFrom<SayaArgs> for SayaConfig {
    type Error = &'static str;

    fn try_from(args: SayaArgs) -> Result<Self, Self::Error> {
        let da_config = match args.data_availability.da_chain {
            Some(chain) => Some(match chain {
                DataAvailabilityChain::Celestia => {
                    let conf = args.data_availability.celestia;

                    DataAvailabilityConfig::Celestia(CelestiaConfig {
                        node_url: match conf.celestia_node_url {
                            Some(v) => v,
                            None => return Err("Celestia config: Node url is required"),
                        },
                        namespace: match conf.celestia_namespace {
                            Some(v) => v,
                            None => return Err("Celestia config: Namespace is required"),
                        },
                        node_auth_token: conf.celestia_node_auth_token,
                    })
                }
            }),
            None => None,
        };

        Ok(SayaConfig {
            katana_rpc: args.rpc_url,
            start_block: args.start_block,
            data_availability: da_config,
        })
    }
}
