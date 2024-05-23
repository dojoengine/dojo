use std::path::PathBuf;

use anyhow::Context;
use prover_sdk::{
    errors::ProverSdkErrors, Cairo0ProverInput, Cairo1CompiledProgram, Cairo1ProverInput,
    CompiledProgram, ProverSDK,
};
use serde_json::Value;
use tokio::{fs::File, io::AsyncReadExt, sync::OnceCell};
use url::Url;

use super::ProveProgram;

pub const CAIRO_VERSION: u8 = 1;

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
    prove_program: ProveProgram,
) -> anyhow::Result<String> {
    let prover = ONCE.get_or_init(|| async { ProverSDK::new(access_key, prover_url).await }).await;
    let prover = prover.as_ref().map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let program_input = Value::Array(vec![Value::String(input)]);
    let mut program = load_program().await?;

    let result = match prove_program {
        // Cairo 0
        ProveProgram::Differ | ProveProgram::Merger => {
            let program: CompiledProgram = serde_json::from_str(&serde_json::to_string(&program)?)?;
            let input = Cairo0ProverInput { program, program_input };

            prover.prove_cairo0(input).await.context("Failed to prove using the http prover")?
        }

        // Cairo 1
        ProveProgram::Universal => {
            if let Value::Object(ref mut obj) = program {
                obj.insert("version".to_string(), Value::Number(serde_json::Number::from(1)));
            }

            let program: Cairo1CompiledProgram =
                serde_json::from_str(&serde_json::to_string(&program)?)?;
            let input = Cairo1ProverInput { program, program_input };

            prover.prove_cairo1(input).await.context("Failed to prove using the http prover")?
        }
    };

    Ok(result)
}
