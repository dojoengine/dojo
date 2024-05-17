use std::{path::PathBuf, sync::Arc};

use anyhow::Context;
use prover_sdk::{errors::ProverSdkErrors, load_cairo1, ProverSDK};
use tokio::sync::{Mutex, OnceCell};
use url::Url;

static ONCE: OnceCell<Mutex<Result<ProverSDK, ProverSdkErrors>>> = OnceCell::const_new();

pub async fn http_prove(
    prover_url: Url,
    access_key: prover_sdk::ProverAccessKey,
    input: String,
) -> anyhow::Result<String> {
    let prover = ONCE
        .get_or_init(|| async { Mutex::new(ProverSDK::new(access_key, prover_url).await) })
        .await
        .lock()
        .await;

    let prover = prover.as_ref().map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("programs/input.json");

    println!("dir: {:?}", dir);

    let input = load_cairo1(dir).await.context("Failed to load cairo1 program")?;
    let result =
        prover.prove_cairo1(input).await.context("Failed to prove using the http prover")?;

    // let client = reqwest::Client::new();
    // let resp = client.post(prover_url).body(input).send().await?;
    // let result = resp.text().await?;
    Ok(result)
}
