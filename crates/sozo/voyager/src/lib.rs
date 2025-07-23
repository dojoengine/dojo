//! Contract verification crate for Dojo migrations.
//!
//! This crate provides functionality to verify deployed Dojo contracts
//! using the Starknet contract verification API. It integrates with the
//! migration process to automatically verify contracts after deployment.

pub mod analyzer;
pub mod client;
pub mod config;
pub mod utils;
pub mod verifier;

// Re-export the main types and traits for convenience
pub use analyzer::ProjectAnalyzer;
pub use client::VerificationClient;
pub use config::{
    ArtifactType, ContractArtifact, FileInfo, ProjectMetadata, VerificationConfig,
    VerificationResult, VerifyJobStatus,
};
pub use utils::{get_project_root, get_project_versions};
pub use verifier::{ContractVerifier, VerificationUi};
