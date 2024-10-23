use std::sync::Arc;

use cairo1_playground::get_cairo_pie;
use herodotus_sharp_playground::SharpSdk;
use prover_sdk::access_key::ProverAccessKey;
use prover_sdk::errors::SdkErrors;
use prover_sdk::sdk::ProverSDK;
use prover_sdk::{JobResponse, ProverResult};
use starknet::core::types::Felt;
use tracing::trace;
use url::Url;

use super::loader::{load_program, prepare_input_cairo};
use super::ProveProgram;
use crate::error::ProverError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpProverParams {
    pub prover_url: Url,
    pub prover_key: ProverAccessKey,
}

pub async fn http_prove(
    prover_params: Arc<HttpProverParams>,
    input: Vec<Felt>,
    prove_program: ProveProgram,
) -> Result<ProverResult, ProverError> {
    let prover =
        ProverSDK::new(prover_params.prover_url.clone(), prover_params.prover_key.clone()).await?;
    let input = prepare_input_cairo(input, prove_program).await?;
    let job_id = prover.prove_cairo(input).await?;
    prover.sse(job_id).await?;
    let response = prover.get_job(job_id).await?;
    let response = response.text().await?;
    let json_response: JobResponse = serde_json::from_str(&response)?;
    if let JobResponse::Completed { result, .. } = json_response {
        Ok(result)
    } else if let JobResponse::Failed { error, .. } = json_response {
        Err(SdkErrors::GetJobResponseError(error).into())
    } else {
        Err(SdkErrors::GetJobResponseError("Prover failed".to_string()).into())
    }
}
pub async fn sharp_prove(
    calls: Vec<Felt>,
    api_key: String,
    prove_program: ProveProgram,
) -> Result<ProverResult, ProverError> {
    let temp_dir = tempdir::TempDir::new("pie_file_path")?;
    let pie_file_path = temp_dir.path().join("pie_file_path.zip");
    let program = load_program(prove_program).await?;
    let program = serde_json::from_value(program)?;
    let output = get_cairo_pie(
        program,
        pie_file_path.clone(),
        cairo1_playground::LayoutName::recursive,
        calls,
    )?;
    trace!("output: {:?}", output);
    let sdk = SharpSdk { api_key };
    let response = sdk
        .proof_generation(
            "recursive".to_string(),
            true,
            pie_file_path.to_str().unwrap().to_string(),
        )
        .await?;

    let proof_path = loop {
        let status = sdk.get_sharp_query_jobs(response.sharp_query_id.clone()).await?;

        if let Some(context) = &status.jobs[0].context {
            if let Some(proof_path) = &context.proof_path {
                break proof_path.clone();
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;
    };

    let proof = sdk.get_proof(proof_path).await?;
    Ok(ProverResult {
        proof: proof.proof,
        serialized_proof: proof.serialized_proof,
        program_hash: proof.program_hash,
        program_output: proof.program_output,
        program_output_hash: proof.program_output_hash,
    })
}
