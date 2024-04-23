// Import necessary modules and structs from external crates.
use clap::Parser;
use katana_primitives::state::StateUpdates;
use katana_primitives::{contract::ContractAddress, FieldElement};
use saya_core::prover::{scheduler::prove_recursively, ProgramInput, ProverIdentifier};
use saya_core::prover::{MessageToAppchain, MessageToStarknet};
use serde_json::{Error, Value};
use std::collections::HashMap;
use std::fs;
use std::str::FromStr;
use tokio::fs::File;
use tokio::io::{stdin, AsyncReadExt, AsyncWriteExt};

// Define the structure for CLI inputs using the clap library.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct CliInput {
    // Field to store file paths received from command-line arguments.
    pub files: Vec<std::path::PathBuf>,
}

// Function to read a JSON file and return its content as a serde_json::Value.
fn read_json_file(file_path: &str) -> Value {
    let data = fs::read_to_string(file_path).expect("Unable to read file");
    serde_json::from_str(&data).expect("Unable to parse JSON")
}

// Function to construct a ProgramInput from JSON data.
fn program_input_from_jsn(json_data: Value) -> ProgramInput {
    ProgramInput {
        // Convert JSON fields to appropriate Rust data types.
        prev_state_root: FieldElement::from(json_data["prev_state_root"].as_u64().unwrap()),
        block_number: json_data["block_number"].as_u64().unwrap(),
        block_hash: FieldElement::from(json_data["block_hash"].as_u64().unwrap()),
        config_hash: FieldElement::from(json_data["config_hash"].as_u64().unwrap()),
        message_to_starknet_segment: vec![MessageToStarknet {
            from_address: ContractAddress::from(
                FieldElement::from_str(&json_data["message_to_starknet_segment"][0].to_string())
                    .unwrap(),
            ),
            to_address: ContractAddress::from(
                FieldElement::from_str(&json_data["message_to_starknet_segment"][1].to_string())
                    .unwrap(),
            ),
            payload: vec![FieldElement::from_str(
                &json_data["message_to_starknet_segment"][2].to_string(),
            )
            .unwrap()],
        }],
        message_to_appchain_segment: vec![MessageToAppchain {
            from_address: ContractAddress::from(
                FieldElement::from_str(&json_data["message_to_appchain_segment"][0].to_string())
                    .unwrap(),
            ),
            to_address: ContractAddress::from(
                FieldElement::from_str(&json_data["message_to_appchain_segment"][1].to_string())
                    .unwrap(),
            ),
            nonce: FieldElement::from_str(&json_data["message_to_appchain_segment"][2].to_string())
                .unwrap(),
            selector: FieldElement::from_str(
                &json_data["message_to_appchain_segment"][3].to_string(),
            )
            .unwrap(),
            payload: vec![FieldElement::from_str(
                &json_data["message_to_appchain_segment"][4].to_string(),
            )
            .unwrap()],
        }],
        // Initialize empty state updates, assuming updates are managed elsewhere or not needed initially.
        state_updates: StateUpdates {
            nonce_updates: HashMap::new(),
            storage_updates: HashMap::new(),
            contract_updates: HashMap::new(),
            declared_classes: HashMap::new(),
        },
    }
}

// Asynchronous function to write results to a JSON file.
async fn prove_to_jsn(result: Vec<String>) {
    let mut file = File::create("result.json").await.expect("Failed to create file");

    // Create and serialize a JSON array from result strings.
    let mut json_array = Vec::new();
    for elem in result.iter() {
        let v: Value = serde_json::from_str(elem).expect("Failed to parse JSON");
        json_array.push(v);
    }
    let serialized = serde_json::to_string_pretty(&json_array).expect("Failed to serialize result");
    file.write_all(serialized.as_bytes()).await.expect("Failed to write to file");
}

// Entry point of the program with async main function to handle I/O operations.
#[tokio::main]
async fn main() {
    let args = CliInput::parse(); // Parse CLI arguments.

    // Error handling for command-line input issues.
    if args.files.is_empty() {
        eprintln!("No files provided");
        std::process::exit(1);
    }
    if !args.files.len().is_power_of_two() {
        eprintln!("Only 2^n files are supported. Got {} files", args.files.len());
        std::process::exit(1);
    }

    // Process each file, converting JSON data to ProgramInput.
    let mut inputs: Vec<ProgramInput> = vec![];
    for file in args.files.iter() {
        let json_data = read_json_file(file.to_str().unwrap());
        let program_input = program_input_from_jsn(json_data);
        inputs.push(program_input);
    }

    // Perform recursive proof generation and write results to a file.
    let result = prove_recursively(inputs, ProverIdentifier::Stone).await.unwrap().0;
    prove_to_jsn(result).await;
}
