use saya_core::prover::scheduler::{prove_recursively};
use saya_core::prover::{prove_stone, ProgramInput, ProverIdentifier};
use katana_primitives::FieldElement;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use serde_json::{json, to_string_pretty};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
#[tokio::main]
async fn main() {
    let inputs = (0..1)
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
    let result = prove_recursively(inputs,ProverIdentifier::Stone).await.unwrap().0;
    // Serialize result to JSON
    let serialized = serde_json::to_string_pretty(&result).expect("Failed to serialize result");
    
    // Write to a file
    let mut file = File::create("result.json").await.expect("Failed to create file");
    file.write_all(serialized.as_bytes()).await.expect("Failed to write to file");
}