use std::sync::Arc;

use anyhow::Context;
use katana_primitives::FieldElement;
use prover_sdk::access_key::ProverAccessKey;
use prover_sdk::sdk::ProverSDK;
use url::Url;

use super::loader::prepare_input_cairo1;
use super::ProveProgram;
use crate::prover::loader::prepare_input_cairo0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpProverParams {
    pub prover_url: Url,
    pub prover_key: ProverAccessKey,
}

pub async fn http_prove_felts(
    prover_params: Arc<HttpProverParams>,
    input: Vec<FieldElement>,
    prove_program: ProveProgram,
) -> anyhow::Result<String> {
    // TODO: might be removed if we can target cairo1 directly, and pass an array of felt.
    let args = input.into_iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",");
    let input = format!("{}", args);

    http_prove(prover_params, input, prove_program).await
}

pub async fn http_prove(
    prover_params: Arc<HttpProverParams>,
    input: String,
    prove_program: ProveProgram,
) -> anyhow::Result<String> {
    let prover =
        ProverSDK::new(prover_params.prover_url.clone(), prover_params.prover_key.clone()).await;
    let prover = prover.as_ref().map_err(|e| anyhow::anyhow!(e.to_string()))?;

    // TODO: cairo0 might be deprectated in the future.
    if prove_program.cairo_version() == FieldElement::ONE {
        let input = prepare_input_cairo1(input, prove_program).await?;
        prover.prove_cairo(input).await.context("Failed to prove using the http prover")
    } else {
        let input = prepare_input_cairo0(input, prove_program).await?;
        prover.prove_cairo0(input).await.context("Failed to prove using the http prover")
    }
}
