// Import necessary modules and structs from external crates.
use clap::Parser;
use katana_primitives::state::StateUpdates;
use katana_primitives::{contract::ContractAddress, FieldElement};
use saya_core::prover::{scheduler::prove_recursively, ProgramInput, ProverIdentifier};
use saya_core::prover::{MessageToAppchain, MessageToStarknet};
use serde_json::{Error, Value, Map};
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
fn program_input_from_json(json_data: Value) -> ProgramInput {
    ProgramInput {
        // Convert JSON fields to appropriate Rust data types.
        prev_state_root: FieldElement::from_str(&json_data["prev_state_root"].to_string()).unwrap(),
        block_number: json_data["block_number"].as_u64().unwrap(),
        block_hash: FieldElement::from_str(&json_data["block_hash"].to_string()).unwrap(),
        config_hash: FieldElement::from_str(&json_data["config_hash"].to_string()).unwrap(),
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
            nonce_updates: HashMap::default(),
            storage_updates: HashMap::default(),
            contract_updates: HashMap::default(),
            declared_classes: HashMap::default(),
        },
    }
}

// Asynchronous function to write results to a JSON file.
// Function to write results to a JSON file, with results labeled by file number.
async fn prove_to_json(result: Vec<String>) {
    let mut file = File::create("result.json").await.expect("Failed to create file");

    // Create a JSON map to hold results with specific keys.
    let mut json_map = Map::new();
    for (index, elem) in result.iter().enumerate() {
        let v: Value = serde_json::from_str(elem).expect("Failed to parse JSON");
        json_map.insert(format!("proof {}", index + 1), v); // Labels start from "proof 1", "proof 2", ...
    }

    // Convert the map into a JSON Value, and then serialize it into a pretty JSON string.
    let serialized = serde_json::to_string_pretty(&json_map).expect("Failed to serialize result");

    // Write the serialized string to the file.
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
    let inputs: Vec<ProgramInput> = args.files.iter()
    .map(|file| {
        let json_data = read_json_file(file.to_str().unwrap());
        program_input_from_json(json_data)
    })
    .collect();

    // Perform recursive proof generation and write results to a file.
    let result = prove_recursively(inputs, ProverIdentifier::Stone).await.unwrap().0;
    prove_to_json(result).await;
}
