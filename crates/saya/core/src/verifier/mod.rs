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
#[derive(Debug)]
pub enum VerifierIdentifier {
    HerodotusStarknetSepoia,
    LocalStoneVerify,
    StarkwareEthereum,
}

pub async fn verify(proof: String, verifier: VerifierIdentifier) -> anyhow::Result<String> {
    match verifier {
        VerifierIdentifier::HerodotusStarknetSepoia => {
            let serialized_proof = parse_proof(proof).unwrap();
            starknet::starknet_verify(serialized_proof).await
        }
        VerifierIdentifier::LocalStoneVerify => crate::prover::local_verify(proof).await,
        VerifierIdentifier::StarkwareEthereum => {
            unimplemented!("Herodotus Starknet not yet supported")
        }
    }
}
