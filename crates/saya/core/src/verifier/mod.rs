//! Verifiers backends.
//!
//! Verifiers are deployed on the verifier layer (chain)
//! where facts and proofs are registered and verified.
//!
//! Verifier implementations are used to provide
//! an interface to query the on-chain verifier, but also
//! submitting facts and proofs.

use ::starknet::core::types::FieldElement;
use serde::{Deserialize, Serialize};

use crate::data_availability::StarknetAccountInput;

mod starknet;

/// Supported verifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerifierIdentifier {
    HerodotusStarknetSepolia(FieldElement),
    StoneLocal,
    StarkwareEthereum,
}

pub async fn verify(
    verifier: VerifierIdentifier,
    serialized_proof: Vec<FieldElement>,
    account: StarknetAccountInput,
) -> anyhow::Result<(String, FieldElement)> {
    match verifier {
        VerifierIdentifier::HerodotusStarknetSepolia(fact_registry_address) => {
            starknet::starknet_verify(fact_registry_address, serialized_proof, account).await
        }
        VerifierIdentifier::StoneLocal => unimplemented!("Stone Verifier not yet supported"),
        VerifierIdentifier::StarkwareEthereum => {
            unimplemented!("Herodotus Starknet not yet supported")
        }
    }
}
