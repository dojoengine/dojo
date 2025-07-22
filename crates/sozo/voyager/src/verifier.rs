//! Contract verifier for handling contract verification during migration

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use starknet_crypto::Felt;
use tokio::time;
use tracing::warn;

use crate::analyzer::ProjectAnalyzer;
use crate::client::VerificationClient;
use crate::config::{
    FileInfo, ProjectMetadata, VerificationConfig, VerificationResult, VerifyJobStatus,
};

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
    pub async fn verify_deployed_contracts<T: VerificationUi>(
        &self,
        ui: &mut T,
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

    async fn wait_for_verification<T: VerificationUi>(
        &self,
        job_id: &str,
        contract_name: &str,
        ui: &mut T,
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

/// Trait for UI updates during verification
pub trait VerificationUi {
    fn update_text_boxed(&mut self, text: String);
}
