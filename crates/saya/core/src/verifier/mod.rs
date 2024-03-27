//! Verifiers backends.
//!
//! Verifiers are deployed on the verifier layer (chain)
//! where facts and proofs are registered and verified.
//!
//! Verifier implementations are used to provide
//! an interface to query the on-chain verifier, but also
//! submitting facts and proofs.

use crate::prover::parse_proof;
mod starknet;

/// Supported verifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VerifierIdentifier {
    #[default]
    HerodotusStarknetSepolia,
    LocalStoneVerify,
    StarkwareEthereum,
}

pub async fn verify(proof: String, verifier: VerifierIdentifier) -> anyhow::Result<String> {
    match verifier {
        VerifierIdentifier::HerodotusStarknetSepolia => {
            let serialized_proof = parse_proof(proof).unwrap();
            starknet::starknet_verify(serialized_proof).await
        }
        VerifierIdentifier::LocalStoneVerify => crate::prover::local_verify(proof).await,
        VerifierIdentifier::StarkwareEthereum => {
            unimplemented!("Herodotus Starknet not yet supported")
        }
    }
}

impl From<&str> for VerifierIdentifier {
    fn from(verifier: &str) -> Self {
        match verifier {
            "herodotus" => VerifierIdentifier::HerodotusStarknetSepolia,
            "local" => VerifierIdentifier::LocalStoneVerify,
            "starkware" => VerifierIdentifier::StarkwareEthereum,
            _ => unreachable!(),
        }
    }
}
