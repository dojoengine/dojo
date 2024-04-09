//! Selecting prover and verifier.
use std::fmt::Display;
use std::str::FromStr;

use anyhow::Result;
use clap::builder::PossibleValue;
use clap::{Args, ValueEnum};
use katana_primitives::FieldElement;
use saya_core::prover::ProverIdentifier;
use saya_core::verifier::VerifierIdentifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prover {
    Stone,
}

impl From<Prover> for ProverIdentifier {
    fn from(p: Prover) -> Self {
        match p {
            Prover::Stone => ProverIdentifier::Stone,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verifier {
    StoneLocal,
    HerodotusStarknetSepolia,
}

impl From<Verifier> for VerifierIdentifier {
    fn from(p: Verifier) -> Self {
        match p {
            Verifier::StoneLocal => VerifierIdentifier::StoneLocal,
            Verifier::HerodotusStarknetSepolia => VerifierIdentifier::HerodotusStarknetSepolia,
        }
    }
}

#[derive(Debug, Args, Clone)]
pub struct ProofOptions {
    #[arg(long)]
    #[arg(help = "Prover to generated the proof from the provable program.")]
    pub prover: Prover,

    #[arg(long)]
    #[arg(help = "Verifier on which the proof should be sent to.")]
    pub verifier: Verifier,

    #[arg(help = "The address of the World contract.")]
    #[arg(long = "world")]
    pub world_address: Option<FieldElement>,
}

// -- Prover.
impl Default for Prover {
    fn default() -> Self {
        Self::Stone
    }
}

impl ValueEnum for Prover {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Stone]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            Self::Stone => Some(PossibleValue::new("stone").alias("Stone")),
        }
    }
}

impl FromStr for Prover {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "stone" | "Stone" => Ok(Self::Stone),
            _ => Err(anyhow::anyhow!("unknown prover: {}", s)),
        }
    }
}

impl Display for Prover {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Prover::Stone => write!(f, "stone"),
        }
    }
}

// -- Verifier.
impl Default for Verifier {
    fn default() -> Self {
        Self::StoneLocal
    }
}

impl ValueEnum for Verifier {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::StoneLocal, Self::HerodotusStarknetSepolia]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            Self::StoneLocal => {
                Some(PossibleValue::new("stone-local").alias("stone_local").alias("StoneLocal"))
            }
            Self::HerodotusStarknetSepolia => Some(
                PossibleValue::new("herodotus_starknet_sepolia")
                    .alias("herodotus-starknet-sepolia")
                    .alias("HerodotusStarknetSepolia"),
            ),
        }
    }
}

impl FromStr for Verifier {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "stone-local" | "stone_local" | "StoneLocal" => Ok(Self::StoneLocal),
            "herodotus-starknet-sepolia"
            | "herodotus_starknet_sepolia"
            | "HerodotusStarknetSepolia" => Ok(Self::HerodotusStarknetSepolia),
            _ => Err(anyhow::anyhow!("unknown verifier: {}", s)),
        }
    }
}

impl Display for Verifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Verifier::StoneLocal => write!(f, "local-stone"),
            Verifier::HerodotusStarknetSepolia => write!(f, "herodotus-starknet-sepolia"),
        }
    }
}
