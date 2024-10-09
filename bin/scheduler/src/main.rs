use std::fs;
use std::sync::Arc;

use clap::Parser;
use katana_primitives::felt::FieldElement;
use saya_core::prover::{HttpProverParams, ProgramInput};
use saya_core::ProverAccessKey;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

#[derive(Parser, Debug, Serialize, Deserialize)]
#[clap(author, version, about, long_about = None)]
pub struct CliInput {
    #[arg(short, long)]
    pub world: Felt,
    #[arg(short, long)]
    pub key: String,
    pub files: Vec<std::path::PathBuf>,
}

fn read_json_file(file_path: &str) -> Value {
    let data = fs::read_to_string(file_path).expect("Unable to read file");
    serde_json::from_str(&data).expect("Unable to parse JSON")
}

fn program_input_from_json(json_data: Value) -> ProgramInput {
    serde_json::from_value(json_data).unwrap()
}

async fn _prove_to_json(result: Vec<String>) {
    let mut file = File::create("result.json").await.expect("Failed to create file");

    let mut json_map = Map::new();
    for (index, elem) in result.iter().enumerate() {
        let v: Value = serde_json::from_str(elem).expect("Failed to parse JSON");
        json_map.insert(format!("proof {}", index + 1), v); // Labels start from "proof 1", "proof 2", ...
    }

    let serialized = serde_json::to_string_pretty(&json_map).expect("Failed to serialize result");

    file.write_all(serialized.as_bytes()).await.expect("Failed to write to file");
}

// Entry point of the program with async main function to handle I/O operations.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use url::Url;
    let args = CliInput::parse(); // Parse CLI arguments.
    if args.files.is_empty() {
        eprintln!("No files provided");
        std::process::exit(1);
    }
    if !args.files.len().is_power_of_two() {
        eprintln!("Only 2^n files are supported. Got {} files", args.files.len());
        std::process::exit(1);
    }

    // Process each file, converting JSON data to ProgramInput.
    let _inputs: Vec<ProgramInput> = args
        .files
        .iter()
        .map(|file| {
            let json_data = read_json_file(file.to_str().unwrap());
            program_input_from_json(json_data)
        })
        .collect();
    let _prover_params = Arc::new(HttpProverParams {
        prover_url: Url::parse("http://localhost:3000").unwrap(),
        prover_key: ProverAccessKey::from_hex_string(&args.key).unwrap(),
    });

    // let (proof, _) =
    //     Scheduler::merge(inputs, args.world,
    // ProverIdentifier::Http(prover_params)).await.unwrap();

    // let proof =
    //     proof.to_felts().into_iter().map(|f| f.to_hex_string()).collect::<Vec<_>>().join(" ");

    // println!("{}", proof);

    Ok(())
}
