//! Verifiers backends.
//!
//! Verifiers are deployed on the verifier layer (chain)
//! where facts and proofs are registered and verified.
//!
//! Verifier implementations are used to provide
//! an interface to query the on-chain verifier, but also
//! submitting facts and proofs.

use ::starknet::core::types::Felt;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::SayaStarknetAccount;

mod starknet;
pub mod utils;

/// Supported verifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerifierIdentifier {
    HerodotusStarknetSepolia(Felt),
    StarkwareEthereum,
}

pub async fn verify(
    verifier: VerifierIdentifier,
    serialized_proof: Vec<Felt>,
    account: &SayaStarknetAccount,
    cairo_version: Felt,
) -> Result<(String, Felt), Error> {
    match verifier {
        VerifierIdentifier::HerodotusStarknetSepolia(fact_registry_address) => {
            starknet::starknet_verify(
                fact_registry_address,
                serialized_proof.clone(),
                cairo_version,
                account,
            )
            .await
        }
        VerifierIdentifier::StarkwareEthereum => {
            unimplemented!("Herodotus Starknet not yet supported")
        }
    }
}
