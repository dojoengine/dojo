use prover_sdk::access_key::ProverAccessKey;
use prover_sdk::errors::SdkErrors;
use prover_sdk::sdk::ProverSDK;
use prover_sdk::{JobResponse, ProverResult};
use starknet::core::types::Felt;
use std::sync::Arc;
use url::Url;

use crate::error::ProverError;

use super::loader::prepare_input_cairo;
use super::ProveProgram;

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
    let job_id =
        prover.prove_cairo(input).await?;
    prover.sse(job_id).await?;
    let response = prover.get_job(job_id).await?;
    let response = response.text().await?;
    let json_response: JobResponse = serde_json::from_str(&response).unwrap();
    if let JobResponse::Completed { result, .. } = json_response {
        return Ok(result);
    }else if let JobResponse::Failed { error, .. } = json_response {
        return Err(SdkErrors::GetJobResponseError(error).into());
    }else{
        return Err(SdkErrors::GetJobResponseError("Prover failed".to_string()).into());
    }
}
