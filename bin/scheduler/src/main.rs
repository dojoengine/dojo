use katana_primitives::FieldElement;
use saya_core::prover::scheduler::prove_recursively;
use saya_core::prover::{prove_stone, ProgramInput, ProverIdentifier};
use serde::{Deserialize, Serialize};
use serde_json::{json, to_string_pretty};
use std::collections::HashMap;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use serde_json::{Value, Error};
struct Proofs(Vec<Value>);
#[tokio::main]
async fn main() {
    let inputs = (0..8)
        .map(|i| ProgramInput {
            prev_state_root: FieldElement::from(i),
            block_number: i,
            block_hash: FieldElement::from(i),
            config_hash: FieldElement::from(i),
            message_to_appchain_segment: Default::default(),
            message_to_starknet_segment: Default::default(),
            state_updates: Default::default(),
        })
        .collect::<Vec<_>>();
    let result = prove_recursively(inputs, ProverIdentifier::Stone).await.unwrap().0;
    // Serialize result to JSON
    //sparsowac to do jsona
    let mut file = File::create("result.json").await.expect("Failed to create file");

    // Create a JSON array to hold all elements
    let mut json_array = Vec::new();

    for elem in result.iter() {
        // Parse each string as JSON and collect into an array
        let v: Value = serde_json::from_str(elem).expect("Failed to parse JSON");
        json_array.push(v);
    }

    // Convert the whole array into a pretty JSON string
    let serialized = serde_json::to_string_pretty(&json_array).expect("Failed to serialize result");

    // Write the pretty JSON string to the file
    file.write_all(serialized.as_bytes()).await.expect("Failed to write to file");

}
