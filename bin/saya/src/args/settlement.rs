use std::fmt::Display;
use std::str::FromStr;

use clap::builder::PossibleValue;
use clap::{Args, ValueEnum};
use starknet::core::types::Felt;
use saya_core::SayaMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SayaModeArg(pub SayaMode);

#[derive(Debug, Args, Clone)]
pub struct SettlementOptions {
    #[arg(help = "Choose either ephemeral or persistent saya mode.")]
    #[arg(long = "mode")]
    pub saya_mode: SayaModeArg,

    #[arg(help = "The settlement contract address.")]
    #[arg(long)]
    pub settlement_contract: Option<Felt>,
}

impl Default for SayaModeArg {
    fn default() -> Self {
        SayaModeArg(SayaMode::Ephemeral)
    }
}

impl ValueEnum for SayaModeArg {
    fn value_variants<'a>() -> &'a [Self] {
        &[SayaModeArg(SayaMode::Ephemeral), SayaModeArg(SayaMode::Persistent)]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self.0 {
            SayaMode::Ephemeral => Some(PossibleValue::new("ephemeral").alias("shard")),
            SayaMode::Persistent => Some(PossibleValue::new("persistent")),
        }
    }
}

impl FromStr for SayaModeArg {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let mode = match s {
            "ephemeral" | "shard" => SayaMode::Ephemeral,
            "persistent" => SayaMode::Persistent,
            _ => Err(anyhow::anyhow!("unknown da chain: {}", s))?,
        };
        Ok(SayaModeArg(mode))
    }
}

impl Display for SayaModeArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            SayaMode::Ephemeral => write!(f, "ephemeral"),
            SayaMode::Persistent => write!(f, "persistent"),
        }
    }
}
