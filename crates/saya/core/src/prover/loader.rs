use std::env;
use std::path::PathBuf;

use prover_sdk::Cairo0ProverInput;
use serde_json::Value;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use super::ProveProgram;

pub async fn load_program(prove_program: ProveProgram) -> anyhow::Result<Value> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let program_file = match prove_program {
        ProveProgram::Differ => manifest_dir.join("programs/cairo0differ.json"),
        ProveProgram::Merger => manifest_dir.join("programs/cairo0merger.json"),
    };
    let mut program_file = File::open(program_file).await?;

    let mut data = String::new();
    program_file.read_to_string(&mut data).await?;
    let json_value: Value = serde_json::from_str(&data)?;

    Ok(json_value)
}

pub async fn prepare_input_cairo0(
    arguments: String,
    prove_program: ProveProgram,
) -> anyhow::Result<Cairo0ProverInput> {
    let program = load_program(prove_program).await?;

    let program = serde_json::from_str(&serde_json::to_string(&program)?)?;
    let program_input: Value = serde_json::from_str(&arguments)?;

    Ok(Cairo0ProverInput { program, program_input, layout: "recursive".into() })
}
