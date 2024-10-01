use prover_sdk::CairoProverInput;
use std::env;
use std::path::PathBuf;

use serde_json::Value;
use starknet_crypto::Felt;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::error::ProverError;

use super::ProveProgram;

pub async fn load_program(prove_program: ProveProgram) -> Result<Value, ProverError> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let program_file = match prove_program {
        ProveProgram::Checker => manifest_dir.join("programs/cairo1checker.json"),
        ProveProgram::Batcher => manifest_dir.join("programs/cairo1batcher.json"),
    };

    let mut program_file = File::open(program_file).await?;

    let mut data = String::new();
    program_file.read_to_string(&mut data).await?;
    let json_value: Value = serde_json::from_str(&data)?;

    Ok(json_value)
}

pub async fn prepare_input_cairo(
    program_input: Vec<Felt>,
    prove_program: ProveProgram,
) -> Result<CairoProverInput, ProverError> {
    let mut program = load_program(prove_program).await?;
    if let Value::Object(ref mut obj) = program {
        obj.insert("version".to_string(), Value::Number(serde_json::Number::from(1)));
    }

    let program = serde_json::from_str(&serde_json::to_string(&program)?)?;

    Ok(CairoProverInput {
        program,
        program_input,
        layout: "recursive".into(),
        n_queries: Some(16),
        pow_bits: Some(20),
    })
}
