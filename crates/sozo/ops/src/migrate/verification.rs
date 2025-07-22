//! Contract verification module for Dojo migrations.
//!
//! This module provides functionality to verify deployed Dojo contracts
//! using the Starknet contract verification API. It integrates with the
//! migration process to automatically verify contracts after deployment.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Result, anyhow};
use reqwest::{Client, StatusCode, multipart};
use serde::Deserialize;
use starknet_crypto::Felt;
use tokio::time;
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
            info!("Adding dojo_version to verification request: {}", dojo_version);
            form = form.text("dojo_version", dojo_version.clone());
        }

        // Add source files using CLI format: files[{path}] for each file
        info!("Adding {} source files to verification request:", files.len());
        info!("=== ALL FILES BEING PROCESSED ===");
        for (i, file) in files.iter().enumerate() {
            info!("  [{}] {} -> {}", i + 1, file.name, file.path.display());
        }
        info!("=== END FILE LIST ===");

        for file in files {
            let content = fs::read_to_string(&file.path)
                .map_err(|e| anyhow!("Failed to read file {}: {}", file.path.display(), e))?;

            let field_name = format!("files[{}]", file.name);
            info!("  Adding form field: {} ({} bytes)", field_name, content.len());

            // Attach the file content under the expected field name
            form = form.text(field_name, content);
        }

        // Debug: Log the complete form data being sent
        info!("=== VERIFICATION PAYLOAD DEBUG ===");
        info!("API URL: {}", url);
        info!("Contract name: {}", contract_name);
        info!("Class hash: {:#066x}", class_hash);
        info!("Package name: {}", metadata.package_name);
        info!("Contract file: {}", metadata.contract_file);
        info!("Build tool: {}", metadata.build_tool);
        if let Some(ref dojo_version) = metadata.dojo_version {
            info!("Dojo version: {}", dojo_version);
        }
        info!("Total files in payload: {}", files.len());

        // Debug: Log raw form field data
        info!("=== RAW FORM FIELDS ===");
        info!("compiler_version: {}", metadata.cairo_version);
        info!("scarb_version: {}", metadata.scarb_version);
        info!("package_name: {}", metadata.package_name);
        info!("name: {}", contract_name);
        info!("contract_file: {}", metadata.contract_file);
        info!("contract-name: {}", contract_name);
        info!("project_dir_path: {}", metadata.project_dir_path);
        info!("build_tool: {}", metadata.build_tool);
        info!("license: MIT");
        if let Some(ref dojo_version) = metadata.dojo_version {
            info!("dojo_version: {}", dojo_version);
        }

        info!("=== FILE FIELDS ===");
        for file in files {
            let content = fs::read_to_string(&file.path)
                .map_err(|e| anyhow!("Failed to read file {}: {}", file.path.display(), e))?;
            info!("files[{}]: {} bytes", file.name, content.len());
            info!("  First 100 chars: {}", &content.chars().take(100).collect::<String>());
            info!(
                "  Last 100 chars: {}",
                &content
                    .chars()
                    .rev()
                    .take(100)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>()
            );
        }
        info!("=== END RAW PAYLOAD DEBUG ===");

        info!("Sending verification request to: {}", url);
        let response = self.client.post(url.clone()).multipart(form).send().await?;

        match response.status() {
            StatusCode::OK => {
                let job_dispatch: VerificationJobDispatch = response.json().await?;
                info!("Contract verification submitted with job ID: {}", job_dispatch.job_id);
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
        let url = self.config.api_url.join(&format!("class-verify/job/{}", job_id))?;
        let response = self.client.get(url).send().await?;

        match response.status() {
            StatusCode::OK => {
                let job: VerificationJob = response.json().await?;
                Ok(job)
            }
            StatusCode::NOT_FOUND => Err(anyhow!("Verification job {} not found", job_id)),
            status => {
                let error_text = response.text().await.unwrap_or_default();
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
        info!(
            "Discovering contract artifacts from manifest file in: {}",
            self.project_root.display()
        );

        // Try to find the manifest file (usually manifest_dev.json for dev profile)
        let manifest_path = self.find_manifest_file()?;
        info!("Reading manifest from: {}", manifest_path.display());

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
            info!("Found contract: {} -> {:#066x}", name, class_hash);

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
            info!("Found model: {} -> {:#066x}", name, class_hash);

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
            info!("Found event: {} -> {:#066x}", name, class_hash);

            artifacts.push(ContractArtifact {
                name,
                class_hash,
                artifact_type: ArtifactType::Event,
            });
        }

        if artifacts.is_empty() {
            return Err(anyhow!("No contract artifacts found in manifest"));
        }

        info!("Discovered {} total artifacts from manifest", artifacts.len());
        Ok(artifacts)
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
        if let Some(package_name) = self.extract_package_name().ok() {
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

    /// Collect source files (without using scarb metadata)
    pub fn collect_source_files(&self, include_tests: bool) -> Result<Vec<FileInfo>> {
        info!("Collecting source files from project root: {}", self.project_root.display());
        info!("Current working directory: {:?}", std::env::current_dir());

        // Check if src directory exists
        let src_dir = self.project_root.join("src");
        info!("Checking for src directory: {} (exists: {})", src_dir.display(), src_dir.exists());

        // List contents of project root
        if let Ok(entries) = fs::read_dir(&self.project_root) {
            info!("Contents of project root:");
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    let file_type = if path.is_dir() { "DIR" } else { "FILE" };
                    info!(
                        "  {} {}",
                        file_type,
                        path.file_name().unwrap_or_default().to_string_lossy()
                    );
                }
            }
        }

        let mut files = Vec::new();

        // Start by recursively collecting all Cairo files from the entire project
        self.collect_all_cairo_files(&self.project_root, &mut files, include_tests)?;

        // Add Scarb.toml if it exists (essential for compilation)
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

        // Validate collected files
        self.validate_files(&files)?;

        info!("Collected {} files for verification", files.len());
        for file in &files {
            info!("  - {} ({})", file.name, file.path.display());
        }

        Ok(files)
    }

    fn collect_all_cairo_files(
        &self,
        dir: &PathBuf,
        files: &mut Vec<FileInfo>,
        include_tests: bool,
    ) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Skip certain directories that are typically not needed for compilation
                let dir_name = path.file_name().unwrap_or_default().to_string_lossy();

                // Always skip target, build artifacts, git, and IDE directories
                if dir_name == "target"
                    || dir_name == ".git"
                    || dir_name == ".vscode"
                    || dir_name == ".idea"
                    || dir_name == "node_modules"
                {
                    continue;
                }

                // Skip test directories if tests are not included
                if !include_tests && (dir_name == "tests" || dir_name == "test") {
                    continue;
                }

                // Recursively process subdirectories
                self.collect_all_cairo_files(&path, files, include_tests)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("cairo") {
                // Skip test files if tests are not included
                if !include_tests && self.is_test_file(&path) {
                    continue;
                }

                let relative_path = path
                    .strip_prefix(&self.project_root)
                    .map_err(|e| {
                        anyhow!("Failed to get relative path for {}: {}", path.display(), e)
                    })?
                    .to_string_lossy()
                    .to_string();
                files.push(FileInfo { name: relative_path, path });
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
        info!("🔍 Finding contract file for: {}", contract_name);

        // Step 1: For Dojo models (m_) and events (e_), use lib.cairo as entry point
        if contract_name.starts_with("m_") || contract_name.starts_with("e_") {
            info!("📋 Using lib.cairo for Dojo model/event: {}", contract_name);
            return Ok("src/lib.cairo".to_string());
        }

        // Step 2: For regular contracts, search for specific files
        let files = self.collect_source_files(false)?;
        info!("📁 Searching through {} source files for contract: {}", files.len(), contract_name);

        // Step 3: Try to find a file that contains the contract definition
        for file in &files {
            if !file.name.ends_with(".cairo") || file.name.contains("test") {
                continue;
            }

            info!("🔎 Checking file: {}", file.name);
            if let Ok(content) = fs::read_to_string(&file.path) {
                // Check if this file contains the contract/struct/trait definition
                if self.file_contains_definition(&content, contract_name) {
                    info!("✅ Found contract definition for {} in: {}", contract_name, file.name);
                    return Ok(file.name.clone());
                } else {
                    info!("❌ No definition found for {} in: {}", contract_name, file.name);
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
                info!("Found filename match for {} in: {}", contract_name, file.name);
                return Ok(file.name.clone());
            }
        }

        // Step 4: Convention-based fallback - look for main entry files
        let conventional_files = ["src/lib.cairo", "src/main.cairo"];
        for conv_file in conventional_files {
            if let Some(file) = files.iter().find(|f| f.name == conv_file) {
                info!("Using conventional entry file for {}: {}", contract_name, file.name);
                return Ok(file.name.clone());
            }
        }

        // Step 5: Use first non-test Cairo file as absolute fallback
        for file in &files {
            if file.name.ends_with(".cairo") && !file.name.contains("test") {
                info!("Using first Cairo file as fallback for {}: {}", contract_name, file.name);
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
        let base_name = if contract_name.starts_with("m_") {
            &contract_name[2..]
        } else if contract_name.starts_with("e_") {
            &contract_name[2..]
        } else {
            contract_name
        };

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

        info!(
            "🔍 Searching for patterns with contract_name='{}', base_name='{}'",
            contract_name, base_name
        );

        // Check exact patterns first
        for pattern in &patterns {
            if content.contains(pattern) {
                info!("✅ Found pattern: '{}'", pattern);
                return true;
            }
        }

        // Check loose patterns
        for pattern in &loose_patterns {
            if content.contains(pattern) {
                info!("✅ Found loose pattern: '{}'", pattern);
                return true;
            }
        }

        info!("❌ No patterns found");
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

    /// Verify contracts from manifest file
    pub async fn verify_deployed_contracts(
        &self,
        ui: &mut MigrationUi,
        cairo_version: &str,
        scarb_version: &str,
    ) -> Result<Vec<VerificationResult>> {
        ui.update_text("Verifying contracts...");

        let mut results = Vec::new();
        let dojo_version = self.analyzer.extract_dojo_version();

        // Discover contracts from manifest
        let artifacts = self.analyzer.discover_contract_artifacts()?;

        // Collect source files once for all contracts
        let files = self.analyzer.collect_source_files(self.config.include_tests)?;

        // Debug: Print all collected files
        info!("Collected {} files for verification:", files.len());
        for file in &files {
            info!("  - {} (path: {})", file.name, file.path.display());
        }

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
                    let result = self.wait_for_verification(&job_id, &artifact.name).await;
                    results.push(result);

                    info!("Verification completed for {}. Proceeding to next...", artifact.name);
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

        info!(
            "Verification metadata for {}: package_name={}, contract_file={}",
            contract_name, metadata.package_name, metadata.contract_file
        );

        self.client.verify_contract(class_hash, contract_name, &metadata, files).await
    }

    async fn wait_for_verification(&self, job_id: &str, contract_name: &str) -> VerificationResult {
        const MAX_ATTEMPTS: u32 = 60; // 5 minutes with 5-second intervals
        const POLL_INTERVAL: Duration = Duration::from_secs(5);

        info!("Waiting for verification of {} (job: {})...", contract_name, job_id);

        for attempt in 1..=MAX_ATTEMPTS {
            match self.client.check_verification_status(job_id).await {
                Ok(job) => match job.status {
                    VerifyJobStatus::Success => {
                        info!("✅ Verification successful for {}", contract_name);
                        return VerificationResult::Verified {
                            job_id: job_id.to_string(),
                            class_hash: job.class_hash.unwrap_or_default(),
                        };
                    }
                    VerifyJobStatus::Fail | VerifyJobStatus::CompileFailed => {
                        warn!(
                            "❌ Verification failed for {}: {}",
                            contract_name,
                            job.message.as_deref().unwrap_or("Unknown error")
                        );
                        return VerificationResult::Failed {
                            contract_name: contract_name.to_string(),
                            error: job.message.unwrap_or_else(|| "Verification failed".to_string()),
                        };
                    }
                    _ => {
                        // Still processing, continue polling
                        if self.config.watch {
                            info!(
                                "⏳ Verification in progress for {} (attempt {}/{}): {:?}",
                                contract_name, attempt, MAX_ATTEMPTS, job.status
                            );
                        }
                        time::sleep(POLL_INTERVAL).await;
                    }
                },
                Err(e) => {
                    warn!("Error checking verification status for {}: {}", contract_name, e);
                    break;
                }
            }
        }

        warn!("⏱️ Verification timeout for {} after {} attempts", contract_name, MAX_ATTEMPTS);
        VerificationResult::Timeout { job_id: job_id.to_string() }
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
