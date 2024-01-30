//! Verifiers backends.
//!
//! Verifiers are deployed on the verifier layer (chain)
//! where facts and proofs are registered and verified.
//!
//! Verifier implementations are used to provide
//! an interface to query the on-chain verifier, but also
//! submitting facts and proofs.

/// Supported verifiers.
#[derive(Debug)]
pub enum VerifierIdentifier {
    StarkwareEthereum,
    HerodotusStarknet,
}
