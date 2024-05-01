//! Verifiers backends.
//!
//! Verifiers are deployed on the verifier layer (chain)
//! where facts and proofs are registered and verified.
//!
//! Verifier implementations are used to provide
//! an interface to query the on-chain verifier, but also
//! submitting facts and proofs.

use std::str::FromStr;

use anyhow::bail;
use serde::{Deserialize, Serialize};

use crate::prover::parse_proof;
mod starknet;

/// Supported verifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VerifierIdentifier {
    #[default]
    HerodotusStarknetSepolia,
    StoneLocal,
    StarkwareEthereum,
}

pub async fn verify(
    proof: String,
    verifier: VerifierIdentifier,
    fact_registry_address: starknet_crypto::FieldElement,
) -> anyhow::Result<String> {
    match verifier {
        VerifierIdentifier::HerodotusStarknetSepolia => {
            let serialized_proof = parse_proof(&proof).unwrap();
            starknet::starknet_verify(fact_registry_address, serialized_proof).await
        }
        VerifierIdentifier::StoneLocal => {
            crate::prover::local_verify(proof, "piniom/verifier:latest").await
        }
        VerifierIdentifier::StarkwareEthereum => {
            unimplemented!("Herodotus Starknet not yet supported")
        }
    }
}

impl FromStr for VerifierIdentifier {
    type Err = anyhow::Error;

    fn from_str(verifier: &str) -> anyhow::Result<Self> {
        Ok(match verifier {
            "herodotus" => VerifierIdentifier::HerodotusStarknetSepolia,
            "local" => VerifierIdentifier::StoneLocal,
            "starkware" => VerifierIdentifier::StarkwareEthereum,
            _ => bail!("Unknown verifier: `{}`.", verifier),
        })
    }
}
