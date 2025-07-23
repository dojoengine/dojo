//! HTTP client for contract verification API

use std::fs;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Result};
use reqwest::{multipart, Client, StatusCode};
use serde_json;
use starknet_crypto::Felt;
use tracing::{debug, warn};

use crate::config::{
    ApiError, FileInfo, ProjectMetadata, VerificationConfig, VerificationJob,
    VerificationJobDispatch, VerificationJobDto,
};

/// Verification-specific error types
#[derive(Debug)]
pub enum VerificationError {
    /// Contract or class is already verified
    AlreadyVerified(String),
    /// Other verification error
    Other(anyhow::Error),
}

impl std::fmt::Display for VerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationError::AlreadyVerified(msg) => write!(f, "{}", msg),
            VerificationError::Other(err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for VerificationError {}

/// Simple circuit breaker for API calls
#[derive(Debug)]
pub(crate) struct CircuitBreaker {
    failure_count: u32,
    last_failure_time: Option<SystemTime>,
    failure_threshold: u32,
    recovery_timeout: Duration,
}

impl CircuitBreaker {
    pub(crate) fn new() -> Self {
        Self {
            failure_count: 0,
            last_failure_time: None,
            failure_threshold: 5, // Open circuit after 5 consecutive failures
            recovery_timeout: Duration::from_secs(60), // Wait 1 minute before retry
        }
    }

    pub(crate) fn should_allow_request(&self) -> bool {
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

    pub(crate) fn record_success(&mut self) {
        self.failure_count = 0;
        self.last_failure_time = None;
    }

    pub(crate) fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(SystemTime::now());
    }
}

/// Verification client for interacting with the verification API
#[derive(Debug)]
pub struct VerificationClient {
    client: Client,
    config: VerificationConfig,
    circuit_breaker: std::sync::Mutex<CircuitBreaker>,
}

impl VerificationClient {
    /// Create a new verification client
    pub fn new(config: VerificationConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout))
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

        Ok(Self { client, config, circuit_breaker: std::sync::Mutex::new(CircuitBreaker::new()) })
    }

    /// Submit a contract for verification
    pub async fn verify_contract(
        &self,
        class_hash: &Felt,
        contract_name: &str,
        metadata: &ProjectMetadata,
        files: &[FileInfo],
    ) -> Result<String, VerificationError> {
        let url = self
            .config
            .api_url
            .join(&format!("class-verify/{:#066x}", class_hash))
            .map_err(|e| VerificationError::Other(anyhow::Error::from(e)))?;

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
            .text("license", metadata.license.as_deref().unwrap_or("MIT").to_string());

        // Add Dojo version if available
        if let Some(ref dojo_version) = metadata.dojo_version {
            debug!("Adding dojo_version to verification request: {}", dojo_version);
            form = form.text("dojo_version", dojo_version.clone());
        }

        // Add source files to verification request
        for file in files {
            let content = fs::read_to_string(&file.path).map_err(|e| {
                VerificationError::Other(anyhow!(
                    "Failed to read file {}: {}",
                    file.path.display(),
                    e
                ))
            })?;

            let field_name = format!("files[{}]", file.name);
            form = form.text(field_name, content);
        }

        debug!("Sending verification request for contract: {}", contract_name);
        let response = self
            .client
            .post(url.clone())
            .multipart(form)
            .send()
            .await
            .map_err(|e| VerificationError::Other(anyhow::Error::from(e)))?;

        match response.status() {
            StatusCode::OK => {
                let job_dispatch: VerificationJobDispatch = response
                    .json()
                    .await
                    .map_err(|e| VerificationError::Other(anyhow::Error::from(e)))?;
                debug!("Contract verification submitted with job ID: {}", job_dispatch.job_id);
                Ok(job_dispatch.job_id)
            }
            StatusCode::BAD_REQUEST => {
                let error: ApiError = response
                    .json()
                    .await
                    .map_err(|e| VerificationError::Other(anyhow::Error::from(e)))?;
                // Check if this is an "already verified" error
                if error.error.contains("Contract or class already verified") {
                    Err(VerificationError::AlreadyVerified(error.error))
                } else {
                    Err(VerificationError::Other(anyhow!(
                        "Verification request failed: {}",
                        error.error
                    )))
                }
            }
            StatusCode::PAYLOAD_TOO_LARGE => Err(VerificationError::Other(anyhow!(
                "Request payload too large. Maximum allowed size is 10MB."
            ))),
            status => {
                let error_text = response.text().await.unwrap_or_default();
                Err(VerificationError::Other(anyhow!(
                    "Verification request failed with status {}: {}",
                    status,
                    error_text
                )))
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
                    "Circuit breaker is open - verification API is experiencing issues. Will \
                     retry after recovery timeout."
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
                        let dto = match serde_json::from_str::<VerificationJobDto>(&response_text) {
                            Ok(d) => d,
                            Err(err) if err.classify() == serde_json::error::Category::Syntax => {
                                let cleaned = self.remove_files_field(&response_text)?;
                                serde_json::from_str::<VerificationJobDto>(&cleaned).map_err(
                                    |e| {
                                        anyhow!(
                                            "Failed to parse verification status response for job \
                                             {} after stripping files: {}",
                                            job_id,
                                            e
                                        )
                                    },
                                )?
                            }
                            Err(err) => {
                                return Err(anyhow!(
                                    "Failed to parse verification status response for job {}: {}",
                                    job_id,
                                    err
                                ));
                            }
                        };

                        let job = VerificationJob::from(dto);

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
        let mut value: serde_json::Value = serde_json::from_str(json_text)?;
        if let Some(obj) = value.as_object_mut() {
            obj.remove("files");
        }
        Ok(serde_json::to_string(&value)?)
    }
}
