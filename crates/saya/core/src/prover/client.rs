use std::sync::Arc;

use anyhow::Context;
use prover_sdk::{ProverSDK, ProverSdkErrors};
use tokio::sync::OnceCell;
use tracing::trace;
use url::Url;

use super::ProveProgram;
use crate::prover::loader::{prepare_input_cairo0, prepare_input_cairo1};
use crate::LOG_TARGET;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpProverParams {
    pub prover_url: Url,
    pub prover_key: prover_sdk::ProverAccessKey,
}

static ONCE: OnceCell<Result<ProverSDK, ProverSdkErrors>> = OnceCell::const_new();

pub async fn http_prove(
    prover_params: Arc<HttpProverParams>,
    input: String,
    prove_program: ProveProgram,
) -> anyhow::Result<String> {
    let prover = ONCE
        .get_or_init(|| async {
            ProverSDK::new(prover_params.prover_key.clone(), prover_params.prover_url.clone()).await
        })
        .await;
    let prover = prover.as_ref().map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let result = if cfg!(feature = "cairo1differ") {
        trace!(target: LOG_TARGET, "Proving with cairo1.");

        let input = prepare_input_cairo1(input, prove_program).await?;
        prover.prove_cairo1(input).await.context("Failed to prove using the http prover")?
    } else {
        trace!(target: LOG_TARGET, "Proving with cairo0.");

        let input = prepare_input_cairo0(input, prove_program).await?;
        prover.prove_cairo0(input).await.context("Failed to prove using the http prover")?
    };

    Ok(result)
}
