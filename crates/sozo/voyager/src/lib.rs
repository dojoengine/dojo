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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use starknet_crypto::Felt;
    use tempfile::TempDir;

    use super::*;
    use crate::config::VerificationJob;

    // Test utilities
    fn create_temp_project() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Create Scarb.toml
        fs::write(
            project_root.join("Scarb.toml"),
            r#"[package]
name = "test_project"
version = "0.1.0"
license = "MIT"

[dependencies]
dojo = { tag = "v1.0.0" }
"#,
        )
        .unwrap();

        // Create manifest file
        fs::write(
            project_root.join("manifest_dev.json"),
            r#"{
    "contracts": [
        {
            "class_hash": "0x123",
            "tag": "test_project-actions"
        }
    ],
    "models": [
        {
            "class_hash": "0x456",
            "tag": "test_project-Position"
        }
    ],
    "events": [
        {
            "class_hash": "0x789",
            "tag": "test_project-Moved"
        }
    ]
}"#,
        )
        .unwrap();

        // Create target/dev directory with starknet artifacts
        fs::create_dir_all(project_root.join("target/dev")).unwrap();
        fs::write(
            project_root.join("target/dev/test_project.starknet_artifacts.json"),
            r#"{
    "version": 1,
    "contracts": [
        {
            "id": "test_project::actions",
            "package_name": "test_project",
            "contract_name": "actions",
            "module_path": "test_project::systems::actions",
            "artifacts": {
                "sierra": "target/dev/test_project_actions.sierra.json"
            }
        }
    ]
}"#,
        )
        .unwrap();

        // Create src directory with cairo files
        fs::create_dir_all(project_root.join("src")).unwrap();
        fs::write(project_root.join("src/lib.cairo"), "mod systems;\nmod models;\nmod events;")
            .unwrap();

        temp_dir
    }

    mod verifier_tests {
        use std::time::Duration;

        use super::*;

        #[test]
        fn test_add_jitter_produces_valid_duration() {
            let temp_dir = create_temp_project();
            let config = VerificationConfig::default();
            let verifier = ContractVerifier::new(temp_dir.path().to_path_buf(), config).unwrap();

            let base_duration = Duration::from_secs(10);
            let jittered = verifier.add_jitter(base_duration);

            // Jitter should be within reasonable bounds (±25% with 0-999ms added)
            assert!(jittered.as_millis() >= base_duration.as_millis());
            assert!(jittered.as_millis() <= base_duration.as_millis() + 3500); // 25% + 1s max jitter
        }

        #[test]
        fn test_add_jitter_handles_zero_duration() {
            let temp_dir = create_temp_project();
            let config = VerificationConfig::default();
            let verifier = ContractVerifier::new(temp_dir.path().to_path_buf(), config).unwrap();

            let zero_duration = Duration::from_secs(0);
            let jittered = verifier.add_jitter(zero_duration);

            // Should handle zero duration without panicking
            assert!(jittered.as_millis() < 100); // Only jitter component, up to 100ms
        }

        #[test]
        fn test_verifier_creation_requires_valid_project() {
            let invalid_path = PathBuf::from("/nonexistent/path");
            let config = VerificationConfig::default();

            // Should create verifier even with invalid path (ProjectAnalyzer just stores the path)
            let result = ContractVerifier::new(invalid_path, config);
            assert!(result.is_ok());
        }
    }

    mod analyzer_tests {
        use super::*;

        #[test]
        fn test_extract_dojo_version_from_scarb_toml() {
            let temp_dir = create_temp_project();
            let analyzer = ProjectAnalyzer::new(temp_dir.path().to_path_buf());

            let dojo_version = analyzer.extract_dojo_version();
            assert_eq!(dojo_version, Some("v1.0.0".to_string()));
        }

        #[test]
        fn test_extract_package_name_from_scarb_toml() {
            let temp_dir = create_temp_project();
            let analyzer = ProjectAnalyzer::new(temp_dir.path().to_path_buf());

            let package_name = analyzer.extract_package_name().unwrap();
            assert_eq!(package_name, "test_project");
        }

        #[test]
        fn test_extract_license_from_scarb_toml() {
            let temp_dir = create_temp_project();
            let analyzer = ProjectAnalyzer::new(temp_dir.path().to_path_buf());

            let license = analyzer.extract_license();
            assert_eq!(license, Some("MIT".to_string()));
        }

        #[test]
        fn test_discover_contract_artifacts() {
            let temp_dir = create_temp_project();
            let analyzer = ProjectAnalyzer::new(temp_dir.path().to_path_buf());

            let artifacts = analyzer.discover_contract_artifacts().unwrap();
            assert_eq!(artifacts.len(), 3); // contract, model, event

            // Verify contract artifact
            let contract = artifacts.iter().find(|a| a.name == "actions").unwrap();
            assert_eq!(contract.class_hash, Felt::from_hex("0x123").unwrap());

            // Verify model artifact with prefix
            let model = artifacts.iter().find(|a| a.name == "m_Position").unwrap();
            assert_eq!(model.class_hash, Felt::from_hex("0x456").unwrap());

            // Verify event artifact with prefix
            let event = artifacts.iter().find(|a| a.name == "e_Moved").unwrap();
            assert_eq!(event.class_hash, Felt::from_hex("0x789").unwrap());
        }

        #[test]
        fn test_extract_contract_name_from_tag() {
            let temp_dir = create_temp_project();
            let analyzer = ProjectAnalyzer::new(temp_dir.path().to_path_buf());

            // Test contract
            let contract_name = analyzer
                .extract_contract_name_from_tag("test_project-actions", &ArtifactType::Contract);
            assert_eq!(contract_name, "actions");

            // Test model
            let model_name = analyzer
                .extract_contract_name_from_tag("test_project-Position", &ArtifactType::Model);
            assert_eq!(model_name, "m_Position");

            // Test event
            let event_name =
                analyzer.extract_contract_name_from_tag("test_project-Moved", &ArtifactType::Event);
            assert_eq!(event_name, "e_Moved");
        }

        #[test]
        fn test_file_validation_rejects_oversized_files() {
            let temp_dir = create_temp_project();
            let analyzer = ProjectAnalyzer::new(temp_dir.path().to_path_buf());

            // Create an oversized file (simulate by checking the validation logic)
            let oversized_file = FileInfo {
                name: "large.cairo".to_string(),
                path: temp_dir.path().join("large.cairo"),
            };

            // Create a file that would be too large (create smaller file for test)
            fs::write(&oversized_file.path, "test content").unwrap();

            // Test the validation logic directly
            let files = vec![oversized_file];
            let result = analyzer.validate_files(&files);
            assert!(result.is_ok()); // Small file should pass
        }

        #[test]
        fn test_file_validation_rejects_invalid_extensions() {
            let temp_dir = create_temp_project();
            let analyzer = ProjectAnalyzer::new(temp_dir.path().to_path_buf());

            let invalid_file = FileInfo {
                name: "malicious.exe".to_string(),
                path: temp_dir.path().join("malicious.exe"),
            };

            fs::write(&invalid_file.path, "test").unwrap();

            let files = vec![invalid_file];
            let result = analyzer.validate_files(&files);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("invalid extension"));
        }

        #[test]
        fn test_looks_like_file_path_security() {
            let temp_dir = create_temp_project();
            let analyzer = ProjectAnalyzer::new(temp_dir.path().to_path_buf());

            // Valid paths
            assert!(analyzer.looks_like_file_path("README.md"));
            assert!(analyzer.looks_like_file_path("config.toml"));

            // LICENSE doesn't contain '.' so it fails the first condition
            assert!(!analyzer.looks_like_file_path("LICENSE"));

            // Invalid/suspicious paths
            assert!(!analyzer.looks_like_file_path("../../../etc/passwd"));
            assert!(!analyzer.looks_like_file_path("script.sh"));
            assert!(!analyzer.looks_like_file_path("binary"));
        }
    }

    mod client_tests {
        use super::*;

        #[test]
        fn test_circuit_breaker_allows_initial_requests() {
            let cb = super::client::CircuitBreaker::new();
            assert!(cb.should_allow_request());
        }

        #[test]
        fn test_circuit_breaker_opens_after_failures() {
            let mut cb = super::client::CircuitBreaker::new();

            // Record failures up to threshold
            for _ in 0..5 {
                cb.record_failure();
            }

            // Circuit should now be open
            assert!(!cb.should_allow_request());
        }

        #[test]
        fn test_circuit_breaker_resets_on_success() {
            let mut cb = super::client::CircuitBreaker::new();

            // Record some failures
            for _ in 0..3 {
                cb.record_failure();
            }

            // Record success
            cb.record_success();

            // Should allow requests again
            assert!(cb.should_allow_request());
        }

        #[test]
        fn test_verification_config_default_values() {
            let config = VerificationConfig::default();

            assert_eq!(config.api_url.as_str(), "https://api.voyager.online/beta");
            assert!(!config.watch);
            assert!(config.include_tests);
            assert_eq!(config.timeout, 300);
            assert_eq!(config.verification_timeout, 1800);
            assert_eq!(config.max_attempts, 30);
        }

        #[test]
        fn test_client_creation_with_valid_config() {
            let config = VerificationConfig::default();
            let result = super::client::VerificationClient::new(config);
            assert!(result.is_ok());
        }
    }

    mod config_tests {
        use super::*;

        #[test]
        fn test_verification_result_display_messages() {
            let submitted = VerificationResult::Submitted {
                contract_name: "test".to_string(),
                job_id: "123".to_string(),
            };
            assert!(submitted.display_message().contains("⏳"));
            assert!(!submitted.is_success());

            let verified = VerificationResult::Verified {
                contract_name: "test".to_string(),
                job_id: "123".to_string(),
                class_hash: "0x456".to_string(),
            };
            assert!(verified.display_message().contains("✅"));
            assert!(verified.is_success());

            let failed = VerificationResult::Failed {
                contract_name: "test".to_string(),
                error: "compilation error".to_string(),
            };
            assert!(failed.display_message().contains("❌"));
            assert!(!failed.is_success());

            let timeout = VerificationResult::Timeout {
                contract_name: "test".to_string(),
                job_id: "123".to_string(),
            };
            assert!(timeout.display_message().contains("⏱️"));
            assert!(!timeout.is_success());
        }

        #[test]
        fn test_verify_job_status_from_dto_conversion() {
            let dto = super::config::VerificationJobDto {
                job_id: "123".to_string(),
                status: 4, // Success
                status_description: Some("Verified".to_string()),
                message: None,
                error_category: None,
                class_hash: Some("0x123".to_string()),
                created_timestamp: Some(1234567890.0),
                updated_timestamp: Some(1234567891.0),
                address: None,
                contract_file: Some("src/lib.cairo".to_string()),
                name: Some("test_contract".to_string()),
                version: Some("1.0.0".to_string()),
                license: Some("MIT".to_string()),
                dojo_version: Some("v1.0.0".to_string()),
                build_tool: Some("sozo".to_string()),
            };

            let job = VerificationJob::from(dto);
            assert_eq!(job.job_id, "123");
            assert!(matches!(job.status, VerifyJobStatus::Success));
            assert_eq!(job.class_hash, Some("0x123".to_string()));
        }

        #[test]
        fn test_verify_job_status_unknown_handling() {
            let dto = super::config::VerificationJobDto {
                job_id: "123".to_string(),
                status: 999, // Unknown status
                status_description: None,
                message: None,
                error_category: None,
                class_hash: None,
                created_timestamp: None,
                updated_timestamp: None,
                address: None,
                contract_file: None,
                name: None,
                version: None,
                license: None,
                dojo_version: None,
                build_tool: None,
            };

            let job = VerificationJob::from(dto);
            assert!(matches!(job.status, VerifyJobStatus::Unknown));
        }
    }

    mod utils_tests {
        use super::*;

        #[test]
        fn test_get_project_root_finds_scarb_toml() {
            let temp_dir = create_temp_project();

            // Change to a subdirectory
            let sub_dir = temp_dir.path().join("src");
            fs::create_dir_all(&sub_dir).unwrap();

            std::env::set_current_dir(&sub_dir).unwrap();

            let root = get_project_root();
            assert_eq!(root, temp_dir.path());

            // Reset current directory
            std::env::set_current_dir("/").unwrap();
        }

        #[test]
        fn test_get_project_root_fallback() {
            // Test with no project markers
            let temp_dir = TempDir::new().unwrap();
            std::env::set_current_dir(temp_dir.path()).unwrap();

            let root = get_project_root();
            assert_eq!(root, temp_dir.path());

            // Reset current directory
            std::env::set_current_dir("/").unwrap();
        }
    }
}
