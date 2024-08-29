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
use tracing::trace;

use crate::SayaStarknetAccount;

mod starknet;
pub mod utils;

/// Supported verifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerifierIdentifier {
    HerodotusStarknetSepolia(Felt),
    StoneLocal,
    StarkwareEthereum,
}

pub async fn verify(
    verifier: VerifierIdentifier,
    serialized_proof: Vec<Felt>,
    account: &SayaStarknetAccount,
    cairo_version: Felt,
) -> anyhow::Result<(String, Felt)> {
    const TRY_LIMIT: usize = 3;
    match verifier {
        VerifierIdentifier::HerodotusStarknetSepolia(fact_registry_address) => {
            let mut tries = 0;
            loop {
                match starknet::starknet_verify(
                    fact_registry_address,
                    serialized_proof.clone(),
                    cairo_version,
                    account,
                )
                .await
                {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        if tries < TRY_LIMIT {
                            trace!("Failed to verify proof: {:?}", e);
                            tries += 1;
                            continue;
                        }
                        break Err(e);
                    }
                }
            }
        }
        VerifierIdentifier::StoneLocal => unimplemented!("Stone Verifier not yet supported"),
        VerifierIdentifier::StarkwareEthereum => {
            unimplemented!("Herodotus Starknet not yet supported")
        }
    }
}
