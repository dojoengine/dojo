//! Data availability options.
use std::fmt::Display;
use std::str::FromStr;

use anyhow::{self, Result};
use clap::builder::PossibleValue;
use clap::{Args, ValueEnum};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataAvailabilityChain {
    Celestia,
}

// TODO: need to be reworked in order to support
// named options (like celestia options should be one
// option).
#[derive(Debug, Args, Clone)]
pub struct DataAvailabilityOptions {
    #[arg(long)]
    #[arg(help = "Data availability chain name")]
    pub da_chain: Option<DataAvailabilityChain>,

    #[command(flatten)]
    #[command(next_help_heading = "Celestia")]
    pub celestia: CelestiaOptions,
}

#[derive(Debug, Args, Clone)]
pub struct CelestiaOptions {
    #[arg(long)]
    #[arg(help = "The node url.")]
    #[arg(requires = "da_chain")]
    #[arg(requires = "celestia_namespace")]
    pub celestia_node_url: Option<Url>,

    #[arg(long)]
    #[arg(help = "An authorization token if required by the node.")]
    #[arg(requires = "celestia_node_url")]
    pub celestia_node_auth_token: Option<String>,

    #[arg(long)]
    #[arg(help = "The namespace used to submit blobs.")]
    #[arg(requires = "celestia_node_url")]
    pub celestia_namespace: Option<String>,
}

// -- Clap enums impls --
//
//
impl Default for DataAvailabilityChain {
    fn default() -> Self {
        Self::Celestia
    }
}

impl ValueEnum for DataAvailabilityChain {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Celestia]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            Self::Celestia => Some(PossibleValue::new("celestia").alias("cel")),
        }
    }
}

impl FromStr for DataAvailabilityChain {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "celestia" | "cel" => Ok(Self::Celestia),
            _ => Err(anyhow::anyhow!("unknown da chain: {}", s)),
        }
    }
}

impl Display for DataAvailabilityChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataAvailabilityChain::Celestia => write!(f, "celestia"),
        }
    }
}
