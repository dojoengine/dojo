use std::collections::HashMap;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

/// Represents the source code for a contract.
#[derive(Debug, Serialize)]
pub struct SourceCode {
    main_file_path: String,
    class_hash: String,
    name: String,
    compiler_version: String,
    is_account_contract: bool,
    files: HashMap<String, String>,
}

/// Represents the API response containing the job_id.
#[derive(Debug, Deserialize)]
pub struct ApiResponse {
    job_id: String,
}

/// Returns the base URL for Starkscan-verifier API depending on the network type.
fn get_starkscan_base_url(network: &str) -> String {
    match network {
        "mainnet" => "https://api.starkscan.co/api",
        "testnet" => "https://api-testnet-2.starkscan.co/api",
        "testnet-2" => "https://api-testnet.starkscan.co/api",
        _ => panic!("Unsupported network"),
    }
}

/// Submits a contract for verification and returns a job_id.
pub async fn submit_verify_class(
    source_code: &SourceCode,
    network: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let base_url = get_starkscan_base_url(network);
    let url = format!("{}/verify_class", base_url);
    let client = Client::new();

    let response = client.post(&url).json(source_code).send().await?;

    if response.status().is_success() {
        let api_response: ApiResponse = response.json().await?;
        Ok(api_response.job_id)
    } else {
        Err(format!("Error: {}", response.status()).into())
    }
}

/// Represents the status of a job.
#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub enum Status {
    PENDING,
    SUCCESS,
    FAILED,
}

/// Represents the job status response.
#[derive(Debug, Deserialize)]
pub struct JobStatusRes {
    class_hash: String,
    status: Status,
    error_message: Option<String>,
}

/// Fetches the job status for a given job_id and network.
pub async fn get_job_status(
    job_id: &str,
    network: &str,
) -> Result<JobStatusRes, Box<dyn std::error::Error>> {
    let base_url = get_starkscan_base_url(network);
    let url = format!("{}/verify_class_job_status/{}", base_url, job_id);
    let client = Client::new();

    let response = client.get(&url).send().await?;

    if response.status().is_success() {
        let job_status: JobStatusRes = response.json().await?;
        Ok(job_status)
    } else {
        Err(format!("Error: {}", response.status()).into())
    }
}

/// Waits for the job to finish and returns the final job status.
async fn wait_for_jobs(
    job_id: &str,
    network: &str,
) -> Result<JobStatusRes, Box<dyn std::error::Error>> {
    loop {
        let job_status = get_job_status(job_id, network).await?;

        match job_status.status {
            Status::SUCCESS | Status::FAILED => return Ok(job_status),
            _ => sleep(Duration::from_secs(3)).await,
        }
    }
}

/// Verifies a contract on a given network.
pub async fn verify_class(
    source_code: &SourceCode,
    network: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Submit job to verify class
    let submit_verify_class_res = submit_verify_class(source_code, network).await;

    let job_id = match submit_verify_class_res {
        Ok(job_id) => job_id,
        Err(err) => {
            return Err(format!(
                "Verifying {} on {} failed. Error: {:?}",
                source_code.name, network, err
            )
            .into());
        }
    };

    // Wait for the job to finish
    let job_status_res = wait_for_jobs(&job_id, network).await;

    match job_status_res {
        Ok(job_status) => {
            match job_status.status {
                Status::SUCCESS => {
                    let starkscan_url = get_starkscan_class_url(&job_status.class_hash, network);
                    println!("{} verified on {}: {}", source_code.name, network, starkscan_url);
                    Ok(())
                }
                Status::FAILED => Err(format!(
                    "Verifying {} on {} failed. Error: {}",
                    source_code.name,
                    network,
                    job_status.error_message.unwrap_or_default()
                )
                .into()),
                _ => Err(format!("Unexpected error verifying {} on {}", source_code.name, network)
                    .into()),
            }
        }
        Err(err) => {
            Err(format!("Unexpected error verifying {}. Error: {:?}", source_code.name, err).into())
        }
    }
}

/// Returns the Starkscan URL for the verified contract.
fn get_starkscan_class_url(class_hash: &str, network: &str) -> String {
    let base_url = get_starkscan_base_url(network);
    format!("{}/contract/{class_hash}", base_url, class_hash = class_hash)
}
