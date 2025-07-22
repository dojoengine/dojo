//! Contract verification module for Dojo migrations.
//!
//! This module provides functionality to verify deployed Dojo contracts
//! using the Starknet contract verification API. It integrates with the
//! migration process to automatically verify contracts after deployment.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use reqwest::{Client, StatusCode, multipart};
use serde::Deserialize;
use serde_json;
use starknet_crypto::Felt;
use tokio::time;
use tracing::{debug, warn};
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
    /// Maximum time to wait for verification completion in seconds
    pub verification_timeout: u64,
    /// Maximum number of retry attempts for status checking
    pub max_attempts: u32,
}

impl Default for VerificationConfig {
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

/// API error response
#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub error: String,
}

/// Simple circuit breaker for API calls
#[derive(Debug)]
struct CircuitBreaker {
    failure_count: u32,
    last_failure_time: Option<SystemTime>,
    failure_threshold: u32,
    recovery_timeout: Duration,
}

impl CircuitBreaker {
    fn new() -> Self {
        Self {
            failure_count: 0,
            last_failure_time: None,
            failure_threshold: 5, // Open circuit after 5 consecutive failures
            recovery_timeout: Duration::from_secs(60), // Wait 1 minute before retry
        }
    }

    fn should_allow_request(&self) -> bool {
        if self.failure_count < self.failure_threshold {
            return true;
        }

        // Check if enough time has passed for recovery
        if let Some(last_failure) = self.last_failure_time {
            if let Ok(elapsed) = last_failure.elapsed() {
                return elapsed > self.recovery_timeout;
            }
        }

        false
    }

    fn record_success(&mut self) {
        self.failure_count = 0;
        self.last_failure_time = None;
    }

    fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(SystemTime::now());
    }
}

/// Verification client for interacting with the verification API
pub struct VerificationClient {
    client: Client,
    config: VerificationConfig,
    circuit_breaker: std::sync::Mutex<CircuitBreaker>,
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
            circuit_breaker: std::sync::Mutex::new(CircuitBreaker::new()),
        }
    }

    /// Submit a contract for verification
    pub async fn verify_contract(
        &self,
        class_hash: &Felt,
        contract_name: &str,
        metadata: &ProjectMetadata,
        files: &[FileInfo],
    ) -> Result<String> {
        let url = self.config.api_url.join(&format!("class-verify/{:#066x}", class_hash))?;

        let mut form = multipart::Form::new()
            .percent_encode_noop()
            .text("compiler_version", metadata.cairo_version.clone())
            .text("scarb_version", metadata.scarb_version.clone())
            .text("package_name", metadata.package_name.clone())
            .text("name", contract_name.to_string())
            .text("contract_file", metadata.contract_file.clone())
            .text("contract-name", contract_name.to_string())
            .text("project_dir_path", metadata.project_dir_path.clone())
            .text("build_tool", metadata.build_tool.clone())
            .text("license", "MIT".to_string()); // Default license

        // Add Dojo version if available
        if let Some(ref dojo_version) = metadata.dojo_version {
            debug!("Adding dojo_version to verification request: {}", dojo_version);
            form = form.text("dojo_version", dojo_version.clone());
        }

        // Add source files to verification request
        for file in files {
            let content = fs::read_to_string(&file.path)
                .map_err(|e| anyhow!("Failed to read file {}: {}", file.path.display(), e))?;

            let field_name = format!("files[{}]", file.name);
            form = form.text(field_name, content);
        }

        debug!("Sending verification request for contract: {}", contract_name);
        let response = self.client.post(url.clone()).multipart(form).send().await?;

        match response.status() {
            StatusCode::OK => {
                let job_dispatch: VerificationJobDispatch = response.json().await?;
                debug!("Contract verification submitted with job ID: {}", job_dispatch.job_id);
                Ok(job_dispatch.job_id)
            }
            StatusCode::BAD_REQUEST => {
                let error: ApiError = response.json().await?;
                Err(anyhow!("Verification request failed: {}", error.error))
            }
            StatusCode::PAYLOAD_TOO_LARGE => {
                Err(anyhow!("Request payload too large. Maximum allowed size is 10MB."))
            }
            status => {
                let error_text = response.text().await.unwrap_or_default();
                Err(anyhow!("Verification request failed with status {}: {}", status, error_text))
            }
        }
    }

    /// Check the status of a verification job
    pub async fn check_verification_status(&self, job_id: &str) -> Result<VerificationJob> {
        // Check circuit breaker before making request
        {
            let cb = self.circuit_breaker.lock().unwrap();
            if !cb.should_allow_request() {
                return Err(anyhow!(
                    "Circuit breaker is open - verification API is experiencing issues. Will retry after recovery timeout."
                ));
            }
        }

        let url = self.config.api_url.join(&format!("class-verify/job/{}", job_id))?;
        let start_time = std::time::Instant::now();

        let result = self.client.get(url).send().await;
        let response_time = start_time.elapsed();

        match result {
            Ok(response) => {
                let status = response.status();
                match status {
                    StatusCode::OK => {
                        // Get response text first to allow debugging on parse failures
                        let response_text = response.text().await?;

                        // Parse selectively to avoid malformed files field
                        let json_value: serde_json::Value = match serde_json::from_str(
                            &response_text,
                        ) {
                            Ok(value) => value,
                            Err(_) => {
                                // If parsing fails due to malformed files field, remove it first
                                match self.remove_files_field(&response_text) {
                                    Ok(cleaned_json) => {
                                        serde_json::from_str(&cleaned_json).map_err(|e| {
                                            anyhow!(
                                                "Failed to parse verification status response for job {}: {}",
                                                job_id,
                                                e
                                            )
                                        })?
                                    },
                                    Err(e) => {
                                        return Err(anyhow!(
                                            "Failed to parse verification status response for job {}: {}",
                                            job_id,
                                            e
                                        ));
                                    }
                                }
                            }
                        };

                        // Extract only the fields we need
                        let job = VerificationJob {
                            job_id: json_value
                                .get("jobid")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string(),
                            status: match json_value.get("status").and_then(|v| v.as_u64()) {
                                Some(0) => VerifyJobStatus::Submitted,
                                Some(1) => VerifyJobStatus::Compiled,
                                Some(2) => VerifyJobStatus::CompileFailed,
                                Some(3) => VerifyJobStatus::Fail,
                                Some(4) => VerifyJobStatus::Success,
                                Some(5) => VerifyJobStatus::InProgress,
                                _ => VerifyJobStatus::Unknown,
                            },
                            status_description: json_value
                                .get("status_description")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            message: json_value
                                .get("message")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            error_category: json_value
                                .get("error_category")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            class_hash: json_value
                                .get("class_hash")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            created_timestamp: json_value
                                .get("created_timestamp")
                                .and_then(|v| v.as_f64()),
                            updated_timestamp: json_value
                                .get("updated_timestamp")
                                .and_then(|v| v.as_f64()),
                            address: json_value
                                .get("address")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            contract_file: json_value
                                .get("contract_file")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            name: json_value
                                .get("name")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            version: json_value
                                .get("version")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            license: json_value
                                .get("license")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            dojo_version: json_value
                                .get("dojo_version")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            build_tool: json_value
                                .get("build_tool")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                        };

                        // Log slow responses for monitoring
                        if response_time > Duration::from_secs(5) {
                            warn!(
                                "Slow verification API response: {}ms for job {}",
                                response_time.as_millis(),
                                job_id
                            );
                        }

                        // Record success for circuit breaker
                        self.circuit_breaker.lock().unwrap().record_success();

                        Ok(job)
                    }
                    StatusCode::NOT_FOUND => {
                        // 404 is not a server error, don't count towards circuit breaker
                        Err(anyhow!("Verification job {} not found", job_id))
                    }
                    status if status.is_server_error() => {
                        // Record server errors for circuit breaker
                        self.circuit_breaker.lock().unwrap().record_failure();
                        let error_text = response.text().await.unwrap_or_default();
                        Err(anyhow!(
                            "Server error checking verification status {}: {}",
                            status,
                            error_text
                        ))
                    }
                    status => {
                        let error_text = response.text().await.unwrap_or_default();
                        Err(anyhow!(
                            "Failed to check verification status {}: {}",
                            status,
                            error_text
                        ))
                    }
                }
            }
            Err(e) => {
                // Record network errors for circuit breaker
                self.circuit_breaker.lock().unwrap().record_failure();
                Err(anyhow!("Network error checking verification status for job {}: {}", job_id, e))
            }
        }
    }

    /// Remove the problematic files field from JSON response
    fn remove_files_field(&self, json_text: &str) -> Result<String> {
        // Find the files field
        if let Some(files_start) = json_text.find(r#""files":"#) {
            // Find the opening brace of the files object
            let search_start = files_start + 8; // length of "files":

            // Skip whitespace to find the opening brace
            let mut object_start = None;
            for (i, c) in json_text[search_start..].char_indices() {
                if c == '{' {
                    object_start = Some(search_start + i);
                    break;
                } else if !c.is_whitespace() {
                    // If we encounter a non-whitespace character that's not '{', bail out
                    return Ok(json_text.to_string());
                }
            }

            if let Some(start) = object_start {
                // Find the matching closing brace
                let mut brace_count = 0;
                let mut in_string = false;
                let mut escaped = false;
                let mut object_end = json_text.len();

                for (i, c) in json_text[start..].char_indices() {
                    if escaped {
                        escaped = false;
                        continue;
                    }

                    match c {
                        '\\' if in_string => escaped = true,
                        '"' => in_string = !in_string,
                        '{' if !in_string => {
                            brace_count += 1;
                        }
                        '}' if !in_string => {
                            brace_count -= 1;
                            if brace_count == 0 {
                                object_end = start + i + 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                }

                // Construct the JSON without the files field
                let before_files = &json_text[0..files_start];
                let after_files =
                    if object_end < json_text.len() { &json_text[object_end..] } else { "" };

                // Clean up commas
                let before_files = before_files.trim_end_matches(',');
                let after_files = after_files.trim_start_matches(',');

                let result = if after_files.is_empty() || after_files.trim_start().starts_with('}')
                {
                    format!("{}{}", before_files, after_files)
                } else {
                    format!("{},{}", before_files, after_files)
                };

                Ok(result)
            } else {
                // No opening brace found, return original
                Ok(json_text.to_string())
            }
        } else {
            // No files field found, return original
            Ok(json_text.to_string())
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

    /// Extract package name from Scarb.toml
    pub fn extract_package_name(&self) -> Result<String> {
        let scarb_toml_path = self.project_root.join("Scarb.toml");

        let contents = fs::read_to_string(&scarb_toml_path)
            .map_err(|e| anyhow!("Failed to read Scarb.toml: {}", e))?;
        let parsed: toml::Value =
            toml::from_str(&contents).map_err(|e| anyhow!("Failed to parse Scarb.toml: {}", e))?;

        // Look for package.name
        parsed
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("No package name found in Scarb.toml"))
    }

    /// Discover contract artifacts from manifest file
    pub fn discover_contract_artifacts(&self) -> Result<Vec<ContractArtifact>> {
        debug!(
            "Discovering contract artifacts from manifest file in: {}",
            self.project_root.display()
        );

        // Try to find the manifest file (usually manifest_dev.json for dev profile)
        let manifest_path = self.find_manifest_file()?;

        let content = fs::read_to_string(&manifest_path).map_err(|e| {
            anyhow!("Failed to read manifest file {}: {}", manifest_path.display(), e)
        })?;

        let manifest: Manifest = serde_json::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse manifest file: {}", e))?;

        let mut artifacts = Vec::new();

        // Add contracts
        for contract in manifest.contracts {
            let class_hash = Felt::from_hex(&contract.class_hash).map_err(|e| {
                anyhow!("Invalid class hash in manifest {}: {}", contract.class_hash, e)
            })?;

            let name = self.extract_contract_name_from_tag(&contract.tag, &ArtifactType::Contract);

            artifacts.push(ContractArtifact {
                name,
                class_hash,
                artifact_type: ArtifactType::Contract,
            });
        }

        // Add models
        for model in manifest.models {
            let class_hash = Felt::from_hex(&model.class_hash).map_err(|e| {
                anyhow!("Invalid class hash in manifest {}: {}", model.class_hash, e)
            })?;

            let name = self.extract_contract_name_from_tag(&model.tag, &ArtifactType::Model);

            artifacts.push(ContractArtifact {
                name,
                class_hash,
                artifact_type: ArtifactType::Model,
            });
        }

        // Add events
        for event in manifest.events {
            let class_hash = Felt::from_hex(&event.class_hash).map_err(|e| {
                anyhow!("Invalid class hash in manifest {}: {}", event.class_hash, e)
            })?;

            let name = self.extract_contract_name_from_tag(&event.tag, &ArtifactType::Event);

            artifacts.push(ContractArtifact {
                name,
                class_hash,
                artifact_type: ArtifactType::Event,
            });
        }

        if artifacts.is_empty() {
            return Err(anyhow!("No contract artifacts found in manifest"));
        }

        Ok(artifacts)
    }

    /// Find and parse the starknet_artifacts.json file
    pub fn find_starknet_artifacts(&self) -> Result<StarknetArtifacts> {
        // Look for starknet_artifacts.json in target/dev directory first
        let target_dev_path = self.project_root.join("target/dev");
        if target_dev_path.exists() {
            // Look for files matching pattern <package_name>.starknet_artifacts.json
            if let Ok(entries) = fs::read_dir(&target_dev_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                        if file_name.ends_with(".starknet_artifacts.json") {
                            let content = fs::read_to_string(&path).map_err(|e| {
                                anyhow!(
                                    "Failed to read starknet artifacts file {}: {}",
                                    path.display(),
                                    e
                                )
                            })?;

                            let artifacts: StarknetArtifacts = serde_json::from_str(&content)
                                .map_err(|e| {
                                    anyhow!("Failed to parse starknet artifacts file: {}", e)
                                })?;

                            debug!("Found starknet artifacts file: {}", path.display());
                            return Ok(artifacts);
                        }
                    }
                }
            }
        }

        Err(anyhow!("No starknet_artifacts.json file found in target/dev directory"))
    }

    /// Find the manifest file (try different naming patterns)
    fn find_manifest_file(&self) -> Result<PathBuf> {
        let possible_names = ["manifest_dev.json", "manifest.json", "manifest_release.json"];

        for name in &possible_names {
            let path = self.project_root.join(name);
            if path.exists() {
                return Ok(path);
            }
        }

        Err(anyhow!("No manifest file found. Expected one of: {:?}", possible_names))
    }

    /// Extract contract name from tag with proper prefixes
    /// e.g., "dojo_starter-actions" -> "actions" (contract)
    /// e.g., "dojo_starter-DirectionsAvailable" -> "m_DirectionsAvailable" (model)
    /// e.g., "dojo_starter-Moved" -> "e_Moved" (event)
    fn extract_contract_name_from_tag(&self, tag: &str, artifact_type: &ArtifactType) -> String {
        if let Ok(package_name) = self.extract_package_name() {
            let prefix = format!("{}-", package_name);
            if let Some(base_name) = tag.strip_prefix(&prefix) {
                return match artifact_type {
                    ArtifactType::Contract => base_name.to_string(),
                    ArtifactType::Model => format!("m_{}", base_name),
                    ArtifactType::Event => format!("e_{}", base_name),
                };
            }
        }

        // Fallback: use the full tag if prefix doesn't match
        tag.to_string()
    }

    /// Collect source files using starknet_artifacts.json (simplified approach)
    pub fn collect_source_files(&self, include_tests: bool) -> Result<Vec<FileInfo>> {
        let mut files = Vec::new();

        // Get the artifacts info to determine what files we need
        let artifacts = self.find_starknet_artifacts()?;

        // Add essential project files
        self.add_essential_project_files(&mut files)?;

        // Add source files referenced by the artifacts
        self.add_source_files_for_artifacts(&artifacts, &mut files, include_tests)?;

        // Validate collected files
        self.validate_files(&files)?;

        debug!("Collected {} files for verification using starknet_artifacts.json", files.len());
        Ok(files)
    }

    /// Add essential project files (Scarb.toml, Scarb.lock, LICENSE, README)
    /// Also includes any files referenced in Scarb.toml
    fn add_essential_project_files(&self, files: &mut Vec<FileInfo>) -> Result<()> {
        // Add Scarb.toml (required for compilation)
        let scarb_toml = self.project_root.join("Scarb.toml");
        if scarb_toml.exists() {
            let relative_path = scarb_toml
                .strip_prefix(&self.project_root)
                .map_err(|e| anyhow!("Failed to get relative path: {}", e))?
                .to_string_lossy()
                .to_string();
            files.push(FileInfo { name: relative_path, path: scarb_toml });
        } else {
            return Err(anyhow!("Scarb.toml not found in project root - required for compilation"));
        }

        // Add Scarb.lock if it exists (helps with reproducible builds)
        let scarb_lock = self.project_root.join("Scarb.lock");
        if scarb_lock.exists() {
            let relative_path = scarb_lock
                .strip_prefix(&self.project_root)
                .map_err(|e| anyhow!("Failed to get relative path: {}", e))?
                .to_string_lossy()
                .to_string();
            files.push(FileInfo { name: relative_path, path: scarb_lock });
        }

        // Add LICENSE file if it exists
        for license_name in &["LICENSE", "COPYING", "NOTICE", "LICENSE.txt", "LICENSE.md"] {
            let license_path = self.project_root.join(license_name);
            if license_path.exists() {
                let relative_path = license_path
                    .strip_prefix(&self.project_root)
                    .map_err(|e| anyhow!("Failed to get relative path: {}", e))?
                    .to_string_lossy()
                    .to_string();
                files.push(FileInfo { name: relative_path, path: license_path });
                break; // Only include the first license file found
            }
        }

        // Add README file if it exists
        for readme_name in &["README.md", "README.txt", "README"] {
            let readme_path = self.project_root.join(readme_name);
            if readme_path.exists() {
                let relative_path = readme_path
                    .strip_prefix(&self.project_root)
                    .map_err(|e| anyhow!("Failed to get relative path: {}", e))?
                    .to_string_lossy()
                    .to_string();
                files.push(FileInfo { name: relative_path, path: readme_path });
                break; // Only include the first README file found
            }
        }

        // Add any files referenced in Scarb.toml
        self.add_scarb_referenced_files(files)?;

        Ok(())
    }

    /// Add files that are referenced in Scarb.toml (like specific README or LICENSE files)
    fn add_scarb_referenced_files(&self, files: &mut Vec<FileInfo>) -> Result<()> {
        let scarb_toml_path = self.project_root.join("Scarb.toml");
        if !scarb_toml_path.exists() {
            return Ok(());
        }

        let contents = fs::read_to_string(&scarb_toml_path)
            .map_err(|e| anyhow!("Failed to read Scarb.toml: {}", e))?;

        let parsed: toml::Value =
            toml::from_str(&contents).map_err(|e| anyhow!("Failed to parse Scarb.toml: {}", e))?;

        let mut added_files = std::collections::HashSet::new();

        // Check for package.readme
        if let Some(readme_path) =
            parsed.get("package").and_then(|p| p.get("readme")).and_then(|r| r.as_str())
        {
            let full_path = self.project_root.join(readme_path);
            if full_path.exists() && added_files.insert(readme_path.to_string()) {
                files.push(FileInfo { name: readme_path.to_string(), path: full_path });
            }
        }

        // Check for package.license-file
        if let Some(license_path) =
            parsed.get("package").and_then(|p| p.get("license-file")).and_then(|l| l.as_str())
        {
            let full_path = self.project_root.join(license_path);
            if full_path.exists() && added_files.insert(license_path.to_string()) {
                files.push(FileInfo { name: license_path.to_string(), path: full_path });
            }
        }

        // Check for any other file references in the TOML that might be important
        // This is a more general approach to catch any file paths mentioned
        self.find_file_references_in_toml(&parsed, "", &mut added_files, files)?;

        Ok(())
    }

    /// Recursively search for file references in TOML structure
    fn find_file_references_in_toml(
        &self,
        value: &toml::Value,
        path_prefix: &str,
        added_files: &mut std::collections::HashSet<String>,
        files: &mut Vec<FileInfo>,
    ) -> Result<()> {
        match value {
            toml::Value::String(s) => {
                // Check if this looks like a file path and the file exists
                if self.looks_like_file_path(s) {
                    let full_path = self.project_root.join(s);
                    if full_path.exists()
                        && self.is_text_file(&full_path)
                        && added_files.insert(s.clone())
                    {
                        files.push(FileInfo { name: s.clone(), path: full_path });
                    }
                }
            }
            toml::Value::Table(table) => {
                for (key, val) in table {
                    let new_prefix = if path_prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", path_prefix, key)
                    };
                    self.find_file_references_in_toml(val, &new_prefix, added_files, files)?;
                }
            }
            toml::Value::Array(arr) => {
                for item in arr {
                    self.find_file_references_in_toml(item, path_prefix, added_files, files)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Check if a string looks like a file path
    fn looks_like_file_path(&self, s: &str) -> bool {
        // Simple heuristics for file paths
        s.contains('.')
            && (s.ends_with(".md")
                || s.ends_with(".txt")
                || s.ends_with(".toml")
                || s.ends_with(".lock")
                || s.starts_with("LICENSE")
                || s.starts_with("README")
                || s.starts_with("COPYING")
                || s.starts_with("NOTICE"))
    }

    /// Check if a file is a text file we should include
    fn is_text_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            matches!(ext, "md" | "txt" | "toml" | "lock" | "cairo")
        } else {
            // Files without extension might be LICENSE, README, etc.
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                name.starts_with("LICENSE")
                    || name.starts_with("README")
                    || name.starts_with("COPYING")
                    || name.starts_with("NOTICE")
            } else {
                false
            }
        }
    }

    /// Add source files based on artifact module paths
    fn add_source_files_for_artifacts(
        &self,
        artifacts: &StarknetArtifacts,
        files: &mut Vec<FileInfo>,
        include_tests: bool,
    ) -> Result<()> {
        let mut added_files = std::collections::HashSet::new();

        // Always add src/lib.cairo as it's the main entry point
        let lib_cairo = self.project_root.join("src/lib.cairo");
        if lib_cairo.exists() {
            let relative_path = "src/lib.cairo".to_string();
            if added_files.insert(relative_path.clone()) {
                files.push(FileInfo { name: relative_path, path: lib_cairo });
            }
        }

        // Analyze module paths from artifacts to determine required source files
        for contract in &artifacts.contracts {
            // Extract potential file paths from module path
            let potential_files = self.extract_file_paths_from_module(&contract.module_path);

            for file_path in potential_files {
                let full_path = self.project_root.join(&file_path);
                if full_path.exists() {
                    // Skip test files if not included
                    if !include_tests && self.is_test_file(&full_path) {
                        continue;
                    }

                    if added_files.insert(file_path.clone()) {
                        files.push(FileInfo { name: file_path, path: full_path });
                    }
                }
            }
        }

        // Add any remaining Cairo files in src/ directory that might be needed
        self.add_remaining_src_files(files, &mut added_files, include_tests)?;

        Ok(())
    }

    /// Extract potential file paths from a module path
    /// e.g., "dojo_starter::models::m_Position" -> ["src/models.cairo", "src/models/mod.cairo"]
    fn extract_file_paths_from_module(&self, module_path: &str) -> Vec<String> {
        let parts: Vec<&str> = module_path.split("::").skip(1).collect(); // Skip package name
        let mut paths = Vec::new();

        if parts.is_empty() {
            return paths;
        }

        // Generate different potential file paths based on common Dojo patterns
        if parts.len() == 1 {
            // Simple case: package::module -> src/module.cairo
            paths.push(format!("src/{}.cairo", parts[0]));
        } else if parts.len() >= 2 {
            // Multi-level: package::systems::actions -> src/systems/actions.cairo, src/systems.cairo, etc.
            for i in 1..parts.len() {
                let file_parts = &parts[0..i];
                let file_name = parts[i - 1];

                if file_parts.len() == 1 {
                    // src/systems.cairo (for systems::actions)
                    paths.push(format!("src/{}.cairo", file_parts[0]));
                } else {
                    // src/systems/mod.cairo or src/systems/actions.cairo
                    let dir_path = file_parts.join("/");
                    paths.push(format!("src/{}/{}.cairo", dir_path, file_name));
                    paths.push(format!("src/{}/mod.cairo", dir_path));
                }
            }

            // Also try the full path as a file
            let full_file_path = parts.join("/");
            paths.push(format!("src/{}.cairo", full_file_path));
        }

        // Always add common files
        if parts.contains(&"models") {
            paths.push("src/models.cairo".to_string());
        }
        if parts.contains(&"systems") {
            paths.push("src/systems.cairo".to_string());
            paths.push("src/systems/mod.cairo".to_string());
        }

        paths.sort();
        paths.dedup();
        paths
    }

    /// Add any remaining Cairo files in src/ that might be needed
    fn add_remaining_src_files(
        &self,
        files: &mut Vec<FileInfo>,
        added_files: &mut std::collections::HashSet<String>,
        include_tests: bool,
    ) -> Result<()> {
        let src_dir = self.project_root.join("src");
        if src_dir.exists() {
            self.collect_remaining_cairo_files(&src_dir, "src", files, added_files, include_tests)?;
        }
        Ok(())
    }

    fn collect_remaining_cairo_files(
        &self,
        dir: &PathBuf,
        relative_prefix: &str,
        files: &mut Vec<FileInfo>,
        added_files: &mut std::collections::HashSet<String>,
        include_tests: bool,
    ) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let dir_name = path.file_name().unwrap_or_default().to_string_lossy();

                // Skip test directories if tests are not included
                if !include_tests && (dir_name == "tests" || dir_name == "test") {
                    continue;
                }

                let new_prefix = format!("{}/{}", relative_prefix, dir_name);
                self.collect_remaining_cairo_files(
                    &path,
                    &new_prefix,
                    files,
                    added_files,
                    include_tests,
                )?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("cairo") {
                // Skip test files if tests are not included
                if !include_tests && self.is_test_file(&path) {
                    continue;
                }

                let relative_path =
                    format!("{}/{}", relative_prefix, path.file_name().unwrap().to_string_lossy());

                if added_files.insert(relative_path.clone()) {
                    files.push(FileInfo { name: relative_path, path });
                }
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
            let file_name = file.path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let allowed_extensions = ["cairo", "toml", "lock", "md", "txt", "json"];
            let allowed_no_extension_files = ["LICENSE", "COPYING", "NOTICE"];

            if !allowed_extensions.contains(&extension)
                && !extension.is_empty()
                && !allowed_no_extension_files.contains(&file_name)
            {
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
        // For Dojo models (m_) and events (e_), use lib.cairo as entry point
        if contract_name.starts_with("m_") || contract_name.starts_with("e_") {
            return Ok("src/lib.cairo".to_string());
        }

        // For regular contracts, search for specific files
        let files = self.collect_source_files(false)?;

        // Step 2: Try to find a file that contains the contract definition
        for file in &files {
            if !file.name.ends_with(".cairo") || file.name.contains("test") {
                continue;
            }

            if let Ok(content) = fs::read_to_string(&file.path) {
                // Check if this file contains the contract/struct/trait definition
                if self.file_contains_definition(&content, contract_name) {
                    return Ok(file.name.clone());
                }
            }
        }

        // Step 3: Try exact filename matches (without extension variations)
        let potential_names = self.generate_potential_filenames(contract_name);
        for file in &files {
            if !file.name.ends_with(".cairo") || file.name.contains("test") {
                continue;
            }

            let file_stem = file.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

            if potential_names.contains(&file_stem.to_string()) {
                return Ok(file.name.clone());
            }
        }

        // Step 4: Convention-based fallback - look for main entry files
        let conventional_files = ["src/lib.cairo", "src/main.cairo"];
        for conv_file in conventional_files {
            if let Some(file) = files.iter().find(|f| f.name == conv_file) {
                return Ok(file.name.clone());
            }
        }

        // Step 5: Use first non-test Cairo file as absolute fallback
        for file in &files {
            if file.name.ends_with(".cairo") && !file.name.contains("test") {
                return Ok(file.name.clone());
            }
        }

        // Final fallback - this should rarely happen
        Err(anyhow!("No suitable contract file found for: {}", contract_name))
    }

    /// Generate potential filenames based on contract name
    fn generate_potential_filenames(&self, contract_name: &str) -> Vec<String> {
        let mut names = vec![contract_name.to_string()];

        // Handle common prefixes
        if let Some(base) = contract_name.strip_prefix("m_") {
            names.push(base.to_string()); // m_Position -> Position
        } else if let Some(base) = contract_name.strip_prefix("e_") {
            names.push(base.to_string()); // e_Moved -> Moved
        } else if let Some(base) = contract_name.strip_prefix("c_") {
            names.push(base.to_string()); // c_Contract -> Contract
        }

        // Add lowercase variations
        names.push(contract_name.to_lowercase());

        // Add snake_case variations
        let snake_case = contract_name
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if i > 0 && c.is_uppercase() {
                    format!("_{}", c.to_lowercase())
                } else {
                    c.to_lowercase().to_string()
                }
            })
            .collect::<String>();
        names.push(snake_case);

        names
    }

    /// Check if a file contains a definition for the given contract name
    fn file_contains_definition(&self, content: &str, contract_name: &str) -> bool {
        // Strip prefixes for pattern matching
        let base_name = contract_name
            .strip_prefix("m_")
            .or_else(|| contract_name.strip_prefix("e_"))
            .unwrap_or(contract_name);

        // Look for various Cairo definition patterns
        let patterns = [
            format!("struct {}", contract_name), // struct m_Position
            format!("struct {}", base_name),     // struct Position
            format!("trait {}", contract_name),  // trait m_Position
            format!("trait {}", base_name),      // trait Position
            format!("mod {}", contract_name),    // mod m_Position
            format!("mod {}", base_name),        // mod Position
            format!("impl {}", contract_name),   // impl m_Position
            format!("impl {}", base_name),       // impl Position
            format!("enum {}", contract_name),   // enum m_Position
            format!("enum {}", base_name),       // enum Position
            format!("#[derive(Model)]\nstruct {}", contract_name), // Dojo model exact
            format!("#[derive(Model)]\nstruct {}", base_name), // Dojo model base
            format!("#[derive(Event)]\nstruct {}", contract_name), // Dojo event exact
            format!("#[derive(Event)]\nstruct {}", base_name), // Dojo event base
        ];

        // Also check for the contract name in comments or exports
        let loose_patterns = [
            format!("// {}", contract_name),
            format!("// {}", base_name),
            format!("pub use {}", contract_name),
            format!("pub use {}", base_name),
            format!("use super::{}", contract_name),
            format!("use super::{}", base_name),
        ];

        // Check exact patterns first
        for pattern in &patterns {
            if content.contains(pattern) {
                return true;
            }
        }

        // Check loose patterns
        for pattern in &loose_patterns {
            if content.contains(pattern) {
                return true;
            }
        }
        false
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

    /// Add jitter to backoff duration to prevent thundering herd
    fn add_jitter(&self, duration: Duration) -> Duration {
        // Use a simple linear congruential generator for jitter
        // This avoids needing external random dependencies
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos() as u64;

        let jitter_ms = seed % 1000; // 0-999ms jitter
        let base_ms = duration.as_millis() as u64;

        // Add ±25% jitter
        let jitter_range = base_ms / 4; // 25% of base duration
        let actual_jitter = (jitter_ms % (jitter_range * 2)).saturating_sub(jitter_range);

        Duration::from_millis(base_ms.saturating_add(actual_jitter))
    }

    /// Verify contracts from manifest file
    pub async fn verify_deployed_contracts(
        &self,
        ui: &mut MigrationUi,
        cairo_version: &str,
        scarb_version: &str,
    ) -> Result<Vec<VerificationResult>> {
        let mut results = Vec::new();
        let dojo_version = self.analyzer.extract_dojo_version();

        // Discover contracts from manifest
        let artifacts = self.analyzer.discover_contract_artifacts()?;

        // Collect source files once for all contracts using the simplified artifacts approach
        let files = self.analyzer.collect_source_files(self.config.include_tests)?;

        for artifact in artifacts {
            ui.update_text_boxed(format!("Verifying {}...", artifact.name));

            match self
                .verify_single_contract(
                    &artifact.class_hash,
                    &artifact.name,
                    cairo_version,
                    scarb_version,
                    &dojo_version,
                    &files,
                )
                .await
            {
                Ok(job_id) => {
                    // Always wait for verification to complete before proceeding to next contract
                    let result = self.wait_for_verification(&job_id, &artifact.name, ui).await;
                    results.push(result);
                }
                Err(e) => {
                    warn!("Failed to verify contract {}: {}", artifact.name, e);
                    results.push(VerificationResult::Failed {
                        contract_name: artifact.name.clone(),
                        error: e.to_string(),
                    });
                }
            }
        }

        Ok(results)
    }

    async fn verify_single_contract(
        &self,
        class_hash: &Felt,
        contract_name: &str,
        cairo_version: &str,
        scarb_version: &str,
        dojo_version: &Option<String>,
        files: &[FileInfo],
    ) -> Result<String> {
        let contract_file = self.analyzer.find_contract_file(contract_name)?;
        let package_name = self.analyzer.extract_package_name()?;

        let metadata = ProjectMetadata {
            cairo_version: cairo_version.to_string(),
            scarb_version: scarb_version.to_string(),
            project_dir_path: ".".to_string(), // Relative to project root
            contract_file,
            package_name,
            build_tool: "sozo".to_string(), // Always sozo for Dojo projects
            dojo_version: dojo_version.clone(),
        };

        self.client.verify_contract(class_hash, contract_name, &metadata, files).await
    }

    async fn wait_for_verification(
        &self,
        job_id: &str,
        contract_name: &str,
        ui: &mut MigrationUi,
    ) -> VerificationResult {
        const INITIAL_INTERVAL: Duration = Duration::from_secs(2);
        const MAX_INTERVAL: Duration = Duration::from_secs(30);
        const BACKOFF_MULTIPLIER: f64 = 1.5;

        let max_attempts = self.config.max_attempts;
        let mut current_interval = INITIAL_INTERVAL;

        for attempt in 1..=max_attempts {
            match self.client.check_verification_status(job_id).await {
                Ok(job) => {
                    match job.status {
                        VerifyJobStatus::Success => {
                            return VerificationResult::Verified {
                                contract_name: contract_name.to_string(),
                                job_id: job_id.to_string(),
                                class_hash: job.class_hash.unwrap_or_default(),
                            };
                        }
                        VerifyJobStatus::Fail | VerifyJobStatus::CompileFailed => {
                            let error_msg = job
                                .message
                                .or_else(|| job.status_description.clone())
                                .unwrap_or_else(|| match job.status {
                                    VerifyJobStatus::CompileFailed => {
                                        "Compilation failed".to_string()
                                    }
                                    VerifyJobStatus::Fail => "Verification failed".to_string(),
                                    _ => "Unknown error".to_string(),
                                });

                            warn!("❌ Verification failed for {}: {}", contract_name, error_msg);
                            return VerificationResult::Failed {
                                contract_name: contract_name.to_string(),
                                error: error_msg,
                            };
                        }
                        _ => {
                            // Still processing, continue polling with backoff
                            ui.update_text_boxed(format!("Verifying {}...", contract_name));

                            // Don't sleep on the last attempt
                            if attempt < max_attempts {
                                let jittered_interval = self.add_jitter(current_interval);
                                time::sleep(jittered_interval).await;

                                // Increase interval for next attempt with exponential backoff
                                current_interval = Duration::from_secs(std::cmp::min(
                                    (current_interval.as_secs() as f64 * BACKOFF_MULTIPLIER) as u64,
                                    MAX_INTERVAL.as_secs(),
                                ));
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Error checking verification status for {}: {}", contract_name, e);

                    // Apply backoff even for errors to avoid overwhelming the API
                    if attempt < max_attempts {
                        let jittered_interval = self.add_jitter(current_interval);
                        time::sleep(jittered_interval).await;
                        current_interval = Duration::from_secs(std::cmp::min(
                            (current_interval.as_secs() as f64 * BACKOFF_MULTIPLIER) as u64,
                            MAX_INTERVAL.as_secs(),
                        ));
                    }
                }
            }
        }

        warn!("Verification timeout for {} after {} attempts", contract_name, max_attempts);
        VerificationResult::Timeout {
            contract_name: contract_name.to_string(),
            job_id: job_id.to_string(),
        }
    }
}

/// Result of a contract verification attempt
#[derive(Debug)]
pub enum VerificationResult {
    /// Verification was submitted successfully
    Submitted { contract_name: String, job_id: String },
    /// Contract was verified successfully
    Verified { contract_name: String, job_id: String, class_hash: String },
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
        matches!(self, Self::Verified { .. })
    }
}

// Tests would go here - removed for now to avoid tempfile dependency
