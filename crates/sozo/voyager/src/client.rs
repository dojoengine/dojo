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
    VerificationJobDispatch, VerifyJobStatus,
};

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
                        let json_value: serde_json::Value =
                            match serde_json::from_str(&response_text) {
                                Ok(value) => value,
                                Err(_) => {
                                    // If parsing fails due to malformed files field, remove it
                                    // first
                                    match self.remove_files_field(&response_text) {
                                        Ok(cleaned_json) => serde_json::from_str(&cleaned_json)
                                            .map_err(|e| {
                                                anyhow!(
                                                    "Failed to parse verification status response \
                                                     for job {}: {}",
                                                    job_id,
                                                    e
                                                )
                                            })?,
                                        Err(e) => {
                                            return Err(anyhow!(
                                                "Failed to parse verification status response for \
                                                 job {}: {}",
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
