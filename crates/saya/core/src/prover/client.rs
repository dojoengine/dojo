use std::path::PathBuf;

use anyhow::Context;
use prover_sdk::{errors::ProverSdkErrors, Cairo1CompiledProgram, Cairo1ProverInput, ProverSDK};
use serde_json::Value;
use tokio::{fs::File, io::AsyncReadExt, sync::OnceCell};
use url::Url;

static ONCE: OnceCell<Result<ProverSDK, ProverSdkErrors>> = OnceCell::const_new();

async fn load_program() -> anyhow::Result<Value> {
    let mut program_file = File::open(
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("programs/differ.json"),
    )
    .await?;
    let mut data = String::new();
    program_file.read_to_string(&mut data).await?;
    let json_value: Value = serde_json::from_str(&data)?;
    Ok(json_value)
}

pub async fn http_prove(
    prover_url: Url,
    access_key: prover_sdk::ProverAccessKey,
    input: String,
) -> anyhow::Result<String> {
    let prover = ONCE.get_or_init(|| async { ProverSDK::new(access_key, prover_url).await }).await;

    let prover = prover.as_ref().map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let mut program = load_program().await?;
    if let Value::Object(ref mut obj) = program {
        obj.insert("version".to_string(), Value::Number(serde_json::Number::from(1)));
    }

    let program: Cairo1CompiledProgram = serde_json::from_str(&serde_json::to_string(&program)?)?;
    let program_input = Value::Array(vec![Value::String(input)]);
    let input = Cairo1ProverInput { program, program_input };

    let result =
        prover.prove_cairo1(input).await.context("Failed to prove using the http prover")?;

    Ok(result)
}
