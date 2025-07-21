//! Contract verification module for Dojo migrations.
//!
//! This module provides functionality to verify deployed Dojo contracts
//! using the Starknet contract verification API. It integrates with the
//! migration process to automatically verify contracts after deployment.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Result, anyhow};
use dojo_utils::LabeledClass;
use reqwest::StatusCode;
use reqwest::blocking::{Client, multipart};
use serde::Deserialize;
use starknet_crypto::Felt;
use tracing::{info, warn};
use url::Url;

use crate::migration_ui::MigrationUi;

/// Configuration for contract verification
#[derive(Debug, Clone)]
pub struct VerificationConfig {
    /// API endpoint URL for verification service
    pub api_url: Url,
    /// Whether to watch verification progress
    pub watch: bool,
    /// Whether to include test files in verification
    pub include_tests: bool,
    /// Timeout for verification requests in seconds
    pub timeout: u64,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            api_url: Url::parse("https://api.voyager.online/beta").unwrap(),
            watch: false,
            include_tests: true,
            timeout: 300,
        }
    }
}

/// Information about a file to be included in verification
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub path: PathBuf,
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
}

/// Verification job status from API
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifyJobStatus {
    Success,
    Fail,
    CompileFailed,
    Processing,
    Submitted,
    Compiled,
    #[serde(other)]
    Unknown,
}

/// API response for verification job submission
#[derive(Debug, Deserialize)]
pub struct VerificationJobDispatch {
    pub job_id: String,
}

/// API response for verification job status
#[derive(Debug, Deserialize)]
pub struct VerificationJob {
    pub job_id: String,
    pub status: VerifyJobStatus,
    pub status_description: Option<String>,
    pub message: Option<String>,
    pub class_hash: Option<String>,
}

/// API error response
#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub error: String,
}

/// Verification client for interacting with the verification API
pub struct VerificationClient {
    client: Client,
    config: VerificationConfig,
}

impl VerificationClient {
    /// Create a new verification client
    pub fn new(config: VerificationConfig) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(config.timeout))
                .build()
                .expect("Failed to create HTTP client"),
            config,
        }
    }

    /// Submit a contract for verification
    pub fn verify_contract(
        &self,
        class_hash: &Felt,
        contract_name: &str,
        metadata: &ProjectMetadata,
        files: &[FileInfo],
    ) -> Result<String> {
        let url = self.config.api_url.join(&format!("class-verify/{:#066x}", class_hash))?;

        let mut form = multipart::Form::new()
            .text("compiler_version", metadata.cairo_version.clone())
            .text("scarb_version", metadata.scarb_version.clone())
            .text("package_name", metadata.package_name.clone())
            .text("name", contract_name.to_string())
            .text("contract_file", metadata.contract_file.clone())
            .text("contract-name", metadata.contract_file.clone())
            .text("project_dir_path", metadata.project_dir_path.clone())
            .text("build_tool", metadata.build_tool.clone())
            .text("license", "MIT".to_string()); // Default license

        // Add Dojo version if available
        if let Some(ref dojo_version) = metadata.dojo_version {
            info!("Adding dojo_version to verification request: {}", dojo_version);
            form = form.text("dojo_version", dojo_version.clone());
        }

        // Add source files
        for file in files {
            let content = fs::read_to_string(&file.path)
                .map_err(|e| anyhow!("Failed to read file {}: {}", file.path.display(), e))?;
            form = form.text(format!("files[{}]", file.name), content);
        }

        let response = self.client.post(url.clone()).multipart(form).send()?;

        match response.status() {
            StatusCode::OK => {
                let job_dispatch: VerificationJobDispatch = response.json()?;
                info!("Contract verification submitted with job ID: {}", job_dispatch.job_id);
                Ok(job_dispatch.job_id)
            }
            StatusCode::BAD_REQUEST => {
                let error: ApiError = response.json()?;
                Err(anyhow!("Verification request failed: {}", error.error))
            }
            StatusCode::PAYLOAD_TOO_LARGE => {
                Err(anyhow!("Request payload too large. Maximum allowed size is 10MB."))
            }
            status => {
                let error_text = response.text().unwrap_or_default();
                Err(anyhow!("Verification request failed with status {}: {}", status, error_text))
            }
        }
    }

    /// Check the status of a verification job
    pub fn check_verification_status(&self, job_id: &str) -> Result<VerificationJob> {
        let url = self.config.api_url.join(&format!("class-verify/job/{}", job_id))?;
        let response = self.client.get(url).send()?;

        match response.status() {
            StatusCode::OK => {
                let job: VerificationJob = response.json()?;
                Ok(job)
            }
            StatusCode::NOT_FOUND => Err(anyhow!("Verification job {} not found", job_id)),
            status => {
                let error_text = response.text().unwrap_or_default();
                Err(anyhow!("Failed to check verification status {}: {}", status, error_text))
            }
        }
    }
}

/// Project analyzer for extracting Dojo project information
pub struct ProjectAnalyzer {
    project_root: PathBuf,
}

impl ProjectAnalyzer {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    /// Extract Dojo version from Scarb.toml
    pub fn extract_dojo_version(&self) -> Option<String> {
        let scarb_toml_path = self.project_root.join("Scarb.toml");

        let contents = fs::read_to_string(&scarb_toml_path).ok()?;
        let parsed: toml::Value = toml::from_str(&contents).ok()?;

        // Look for dependencies.dojo.tag
        parsed.get("dependencies")?.get("dojo")?.get("tag")?.as_str().map(|s| s.to_string())
    }

    /// Collect source files for verification
    pub fn collect_source_files(&self, include_tests: bool) -> Result<Vec<FileInfo>> {
        let mut files = Vec::new();
        let src_dir = self.project_root.join("src");

        if src_dir.exists() {
            self.collect_cairo_files(&src_dir, &mut files, include_tests)?;
        }

        // Add manifest files
        let scarb_toml = self.project_root.join("Scarb.toml");
        if scarb_toml.exists() {
            files.push(FileInfo { name: "Scarb.toml".to_string(), path: scarb_toml });
        }

        // Add lock file if it exists
        let scarb_lock = self.project_root.join("Scarb.lock");
        if scarb_lock.exists() {
            files.push(FileInfo { name: "Scarb.lock".to_string(), path: scarb_lock });
        }

        // Validate files
        self.validate_files(&files)?;

        Ok(files)
    }

    fn collect_cairo_files(
        &self,
        dir: &PathBuf,
        files: &mut Vec<FileInfo>,
        include_tests: bool,
    ) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Skip test directories if tests are not included
                if !include_tests && path.file_name().unwrap_or_default() == "tests" {
                    continue;
                }
                self.collect_cairo_files(&path, files, include_tests)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("cairo") {
                // Skip test files if tests are not included
                if !include_tests && self.is_test_file(&path) {
                    continue;
                }

                let relative_path = path.strip_prefix(&self.project_root)?;
                files.push(FileInfo { name: relative_path.to_string_lossy().to_string(), path });
            }
        }
        Ok(())
    }

    fn is_test_file(&self, path: &Path) -> bool {
        path.to_string_lossy().contains("test")
            || path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("test_") || n.ends_with("_test.cairo"))
                .unwrap_or(false)
    }

    fn validate_files(&self, files: &[FileInfo]) -> Result<()> {
        const MAX_FILE_SIZE: u64 = 20 * 1024 * 1024; // 20MB

        for file in files {
            // Validate file size
            let metadata = fs::metadata(&file.path)?;
            if metadata.len() > MAX_FILE_SIZE {
                return Err(anyhow!(
                    "File {} exceeds maximum size limit of {}MB",
                    file.path.display(),
                    MAX_FILE_SIZE / (1024 * 1024)
                ));
            }

            // Validate file type
            let extension = file.path.extension().and_then(|s| s.to_str()).unwrap_or("");
            let allowed_extensions = ["cairo", "toml", "lock", "md", "txt", "json"];

            if !allowed_extensions.contains(&extension) && !extension.is_empty() {
                return Err(anyhow!(
                    "File {} has invalid extension: {}",
                    file.path.display(),
                    extension
                ));
            }
        }

        Ok(())
    }

    /// Find the main contract file for a given contract name
    pub fn find_contract_file(&self, contract_name: &str) -> Result<String> {
        // Try common contract file patterns
        let possible_paths = vec![
            format!("src/{}.cairo", contract_name),
            format!("src/systems/{}.cairo", contract_name),
            format!("src/contracts/{}.cairo", contract_name),
            "src/lib.cairo".to_string(),
            "src/main.cairo".to_string(),
        ];

        for path_str in possible_paths {
            let path = self.project_root.join(&path_str);
            if path.exists() {
                return Ok(path_str);
            }
        }

        // Default to lib.cairo if nothing found
        Ok("src/lib.cairo".to_string())
    }
}

/// Verifier for handling contract verification during migration
pub struct ContractVerifier {
    client: VerificationClient,
    analyzer: ProjectAnalyzer,
    config: VerificationConfig,
}

impl ContractVerifier {
    /// Create a new contract verifier
    pub fn new(project_root: PathBuf, config: VerificationConfig) -> Self {
        let client = VerificationClient::new(config.clone());
        let analyzer = ProjectAnalyzer::new(project_root);

        Self { client, analyzer, config }
    }

    /// Verify contracts that were declared during migration
    pub async fn verify_declared_contracts(
        &self,
        ui: &mut MigrationUi,
        declared_classes: &HashMap<Felt, LabeledClass>,
        cairo_version: &str,
        scarb_version: &str,
    ) -> Result<Vec<VerificationResult>> {
        ui.update_text("Verifying contracts...");

        let mut results = Vec::new();
        let dojo_version = self.analyzer.extract_dojo_version();

        // Collect source files once for all contracts
        let files = self.analyzer.collect_source_files(self.config.include_tests)?;

        for (class_hash, labeled_class) in declared_classes {
            // Only verify contracts (skip models, events, etc.)
            if !self.is_contract(&labeled_class.label) {
                continue;
            }

            let contract_name = self.extract_contract_name(&labeled_class.label);

            ui.update_text("Verifying contract...");

            match self.verify_single_contract(
                class_hash,
                &contract_name,
                cairo_version,
                scarb_version,
                &dojo_version,
                &files,
            ) {
                Ok(job_id) => {
                    let result = if self.config.watch {
                        self.wait_for_verification(&job_id).await
                    } else {
                        VerificationResult::Submitted { job_id }
                    };
                    results.push(result);
                }
                Err(e) => {
                    warn!("Failed to verify contract {}: {}", contract_name, e);
                    results.push(VerificationResult::Failed {
                        contract_name: contract_name.clone(),
                        error: e.to_string(),
                    });
                }
            }
        }

        Ok(results)
    }

    fn verify_single_contract(
        &self,
        class_hash: &Felt,
        contract_name: &str,
        cairo_version: &str,
        scarb_version: &str,
        dojo_version: &Option<String>,
        files: &[FileInfo],
    ) -> Result<String> {
        let contract_file = self.analyzer.find_contract_file(contract_name)?;

        let metadata = ProjectMetadata {
            cairo_version: cairo_version.to_string(),
            scarb_version: scarb_version.to_string(),
            project_dir_path: ".".to_string(), // Relative to project root
            contract_file,
            package_name: contract_name.to_string(),
            build_tool: "sozo".to_string(), // Always sozo for Dojo projects
            dojo_version: dojo_version.clone(),
        };

        self.client.verify_contract(class_hash, contract_name, &metadata, files)
    }

    async fn wait_for_verification(&self, job_id: &str) -> VerificationResult {
        const MAX_ATTEMPTS: u32 = 60; // 5 minutes with 5-second intervals
        const POLL_INTERVAL: Duration = Duration::from_secs(5);

        for _ in 0..MAX_ATTEMPTS {
            match self.client.check_verification_status(job_id) {
                Ok(job) => match job.status {
                    VerifyJobStatus::Success => {
                        return VerificationResult::Verified {
                            job_id: job_id.to_string(),
                            class_hash: job.class_hash.unwrap_or_default(),
                        };
                    }
                    VerifyJobStatus::Fail | VerifyJobStatus::CompileFailed => {
                        return VerificationResult::Failed {
                            contract_name: job_id.to_string(),
                            error: job.message.unwrap_or_else(|| "Verification failed".to_string()),
                        };
                    }
                    _ => {
                        // Still processing, continue polling
                        tokio::time::sleep(POLL_INTERVAL).await;
                    }
                },
                Err(e) => {
                    warn!("Error checking verification status: {}", e);
                    break;
                }
            }
        }

        VerificationResult::Timeout { job_id: job_id.to_string() }
    }

    fn is_contract(&self, label: &str) -> bool {
        // Check if this is a contract (not a model, event, etc.)
        // Dojo contracts typically have specific naming patterns
        !label.contains("::model::")
            && !label.contains("::event::")
            && !label.contains("::interface::")
    }

    fn extract_contract_name(&self, label: &str) -> String {
        // Extract contract name from label
        // Labels typically look like "namespace::contract_name" or just "contract_name"
        label.split("::").last().unwrap_or(label).to_string()
    }
}

/// Result of a contract verification attempt
#[derive(Debug)]
pub enum VerificationResult {
    /// Verification was submitted successfully
    Submitted { job_id: String },
    /// Contract was verified successfully
    Verified { job_id: String, class_hash: String },
    /// Verification failed
    Failed { contract_name: String, error: String },
    /// Verification timed out
    Timeout { job_id: String },
}

impl VerificationResult {
    /// Get a display message for this result
    pub fn display_message(&self) -> String {
        match self {
            Self::Submitted { job_id } => format!("⏳ Submitted (job: {})", job_id),
            Self::Verified { class_hash, .. } => format!("✅ Verified (class: {})", class_hash),
            Self::Failed { error, .. } => format!("❌ Failed: {}", error),
            Self::Timeout { job_id } => format!("⏱️ Timeout (job: {})", job_id),
        }
    }

    /// Check if this result represents a successful verification
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Verified { .. })
    }
}

// Tests would go here - removed for now to avoid tempfile dependency
