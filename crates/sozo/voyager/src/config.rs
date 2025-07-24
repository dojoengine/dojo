//! Configuration types for contract verification

use std::path::PathBuf;

use serde::Deserialize;
use starknet_crypto::Felt;
use url::Url;

/// Configuration for contract verification services
#[derive(Debug, Clone)]
pub enum VerificationConfig {
    /// No verification enabled
    None,
    /// Voyager verification service
    Voyager(VoyagerConfig),
}

/// Configuration specific to Voyager verification service
#[derive(Debug, Clone)]
pub struct VoyagerConfig {
    /// API endpoint URL for Voyager service
    pub api_url: Url,
    /// Whether to watch verification progress
    pub watch: bool,
    /// Whether to include test files in verification
    pub include_tests: bool,
    /// Timeout for verification requests in seconds
    pub timeout: u64,
    /// Maximum time to wait for verification completion in seconds
    pub verification_timeout: u64,
    /// Maximum number of retry attempts for status checking
    pub max_attempts: u32,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self::None
    }
}

impl Default for VoyagerConfig {
    fn default() -> Self {
        Self {
            api_url: Url::parse("https://api.voyager.online/beta").unwrap(),
            watch: false,
            include_tests: true,
            timeout: 300,               // 5 minutes for HTTP requests
            verification_timeout: 1800, // 30 minutes total for verification
            max_attempts: 30,
        }
    }
}

impl VoyagerConfig {
    /// Create a new VoyagerConfig with the specified API URL
    pub fn new(api_url: Url, watch: bool) -> Self {
        Self {
            api_url,
            watch,
            include_tests: true,
            timeout: 300,
            verification_timeout: 1800,
            max_attempts: 30,
        }
    }
}

/// Information about a file to be included in verification
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub path: PathBuf,
}

/// Contract artifact information from manifest
#[derive(Debug, Clone)]
pub struct ContractArtifact {
    pub name: String,
    pub class_hash: Felt,
    pub artifact_type: ArtifactType,
}

/// Type of artifact (contract, model, event)
#[derive(Debug, Clone)]
pub enum ArtifactType {
    Contract,
    Model,
    Event,
}

/// Project metadata for verification API
#[derive(Debug, Clone)]
pub struct ProjectMetadata {
    pub cairo_version: String,
    pub scarb_version: String,
    pub project_dir_path: String,
    pub contract_file: String,
    pub package_name: String,
    pub build_tool: String,
    pub dojo_version: Option<String>,
    pub license: Option<String>,
}

/// Verification job status from API (numeric values)
#[derive(Debug, Deserialize)]
pub enum VerifyJobStatus {
    #[serde(rename = "0")]
    Submitted,
    #[serde(rename = "1")]
    Compiled,
    #[serde(rename = "2")]
    CompileFailed,
    #[serde(rename = "3")]
    Fail,
    #[serde(rename = "4")]
    Success,
    #[serde(rename = "5")]
    InProgress,
    #[serde(other)]
    Unknown,
}

/// API response for verification job submission
#[derive(Debug, Deserialize)]
pub struct VerificationJobDispatch {
    pub job_id: String,
}

/// DTO for deserializing API response matching exact JSON field names
#[derive(Debug, Deserialize)]
pub struct VerificationJobDto {
    #[serde(rename = "jobid")]
    pub job_id: String,
    pub status: u64,
    #[serde(rename = "status_description")]
    pub status_description: Option<String>,
    pub message: Option<String>,
    #[serde(rename = "error_category")]
    pub error_category: Option<String>,
    #[serde(rename = "class_hash")]
    pub class_hash: Option<String>,
    pub created_timestamp: Option<f64>,
    pub updated_timestamp: Option<f64>,
    pub address: Option<String>,
    pub contract_file: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub license: Option<String>,
    pub dojo_version: Option<String>,
    pub build_tool: Option<String>,
}

/// API response for verification job status  
#[derive(Debug, Deserialize)]
pub struct VerificationJob {
    pub job_id: String,
    pub status: VerifyJobStatus,
    #[serde(default)]
    pub status_description: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub error_category: Option<String>,
    #[serde(default)]
    pub class_hash: Option<String>,
    #[serde(default)]
    pub created_timestamp: Option<f64>,
    #[serde(default)]
    pub updated_timestamp: Option<f64>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub contract_file: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub dojo_version: Option<String>,
    #[serde(default)]
    pub build_tool: Option<String>,
}

impl From<VerificationJobDto> for VerificationJob {
    fn from(dto: VerificationJobDto) -> Self {
        Self {
            job_id: dto.job_id,
            status: match dto.status {
                0 => VerifyJobStatus::Submitted,
                1 => VerifyJobStatus::Compiled,
                2 => VerifyJobStatus::CompileFailed,
                3 => VerifyJobStatus::Fail,
                4 => VerifyJobStatus::Success,
                5 => VerifyJobStatus::InProgress,
                _ => VerifyJobStatus::Unknown,
            },
            status_description: dto.status_description,
            message: dto.message,
            error_category: dto.error_category,
            class_hash: dto.class_hash,
            created_timestamp: dto.created_timestamp,
            updated_timestamp: dto.updated_timestamp,
            address: dto.address,
            contract_file: dto.contract_file,
            name: dto.name,
            version: dto.version,
            license: dto.license,
            dojo_version: dto.dojo_version,
            build_tool: dto.build_tool,
        }
    }
}

/// API error response
#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub error: String,
}

/// Starknet artifacts file structure
#[derive(Debug, Deserialize)]
pub struct StarknetArtifacts {
    pub version: u32,
    pub contracts: Vec<ArtifactContract>,
}

#[derive(Debug, Deserialize)]
pub struct ArtifactContract {
    pub id: String,
    pub package_name: String,
    pub contract_name: String,
    pub module_path: String,
    pub artifacts: ArtifactFiles,
}

#[derive(Debug, Deserialize)]
pub struct ArtifactFiles {
    pub sierra: String,
    pub casm: Option<String>,
}

/// Manifest file structure (simplified)
#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub contracts: Vec<ManifestContract>,
    pub models: Vec<ManifestModel>,
    pub events: Vec<ManifestEvent>,
}

#[derive(Debug, Deserialize)]
pub struct ManifestContract {
    pub class_hash: String,
    pub tag: String,
}

#[derive(Debug, Deserialize)]
pub struct ManifestModel {
    pub class_hash: String,
    pub tag: String,
}

#[derive(Debug, Deserialize)]
pub struct ManifestEvent {
    pub class_hash: String,
    pub tag: String,
}

/// Result of a contract verification attempt
#[derive(Debug)]
pub enum VerificationResult {
    /// Verification was submitted successfully
    Submitted { contract_name: String, job_id: String },
    /// Contract was verified successfully
    Verified { contract_name: String, job_id: String, class_hash: String },
    /// Contract was already verified
    AlreadyVerified { contract_name: String, class_hash: String },
    /// Verification failed
    Failed { contract_name: String, error: String },
    /// Verification timed out
    Timeout { contract_name: String, job_id: String },
}

impl VerificationResult {
    /// Get a display message for this result
    pub fn display_message(&self) -> String {
        match self {
            Self::Submitted { contract_name, job_id } => {
                format!("⏳ Submitted {} (job: {})", contract_name, job_id)
            }
            Self::Verified { contract_name, class_hash, .. } => {
                format!("✅ Verified {} (class: {})", contract_name, class_hash)
            }
            Self::AlreadyVerified { contract_name, class_hash } => {
                format!("✅ Already verified {} (class: {})", contract_name, class_hash)
            }
            Self::Failed { contract_name, error } => {
                format!("❌ Failed {}: {}", contract_name, error)
            }
            Self::Timeout { contract_name, job_id } => {
                format!("⏱️ Timeout {} (job: {})", contract_name, job_id)
            }
        }
    }

    /// Check if this result represents a successful verification
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Verified { .. } | Self::AlreadyVerified { .. })
    }
}
