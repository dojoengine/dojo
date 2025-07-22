//! Project analyzer for extracting Dojo project information

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use serde_json;
use starknet_crypto::Felt;
use tracing::debug;

use crate::config::{ArtifactType, ContractArtifact, FileInfo, Manifest, StarknetArtifacts};

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
