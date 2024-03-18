//! Verifiers backends.
//!
//! Verifiers are deployed on the verifier layer (chain)
//! where facts and proofs are registered and verified.
//!
//! Verifier implementations are used to provide
//! an interface to query the on-chain verifier, but also
//! submitting facts and proofs.

use ::starknet::core::types::FieldElement;
mod starknet;

/// Supported verifiers.
#[derive(Debug)]
pub enum VerifierIdentifier {
    StarkwareEthereum,
    HerodotusStarknet,
}

pub async fn verify(
    serialized_proof: Vec<FieldElement>,
    verifier: VerifierIdentifier,
) -> anyhow::Result<String> {
    match verifier {
        VerifierIdentifier::StarkwareEthereum => starknet::starknet_verify(serialized_proof).await,
        VerifierIdentifier::HerodotusStarknet => {
            unimplemented!("Herodotus Starknet not yet supported")
        }
    }
}
