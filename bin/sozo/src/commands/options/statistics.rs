use std::fs::{self, File};
use std::path::PathBuf;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::FlattenedSierraClass;

pub struct ContractStatistics {
    pub contract_name: String,
    pub number_felts: u64,
    pub file_size: u64,
}

#[derive(Debug, Args)]
#[group(required = false, multiple = false)]
pub struct Stats {
    #[arg(long, help = "Display statistics")]
    pub stats: bool,

    #[arg(
        long,
        value_name = "FILE",
        help = "Specify a JSON file with custom limits for statistics"
    )]
    pub stats_limits: Option<String>,
}

pub fn read_sierra_json_program(file: &File) -> Result<FlattenedSierraClass> {
    let contract_artifact: SierraClass = serde_json::from_reader(file)?;
    let contract_artifact: FlattenedSierraClass = contract_artifact.flatten()?;

    Ok(contract_artifact)
}

pub fn compute_contract_byte_code_size(contract_artifact: FlattenedSierraClass) -> usize {
    contract_artifact.sierra_program.len()
}

pub fn get_file_size_in_bytes(file: File) -> u64 {
    file.metadata().unwrap().len()
}

pub fn get_contract_statistics_for_file(
    file_name: String,
    sierra_json_file: File,
    contract_artifact: FlattenedSierraClass,
) -> ContractStatistics {
    ContractStatistics {
        contract_name: file_name,
        number_felts: compute_contract_byte_code_size(contract_artifact) as u64,
        file_size: get_file_size_in_bytes(sierra_json_file),
    }
}

pub fn get_contract_statistics_for_dir(target_directory: &Utf8PathBuf) -> Vec<ContractStatistics> {
    let mut contract_statistics = Vec::new();
    let built_contract_paths: fs::ReadDir = fs::read_dir(target_directory.as_str()).unwrap();
    for sierra_json_path in built_contract_paths {
        let sierra_json_path: PathBuf = sierra_json_path.unwrap().path();

        let sierra_json_file: File = match File::open(&sierra_json_path) {
            Ok(file) => file,
            Err(_) => {
                println!("Error opening Sierra JSON file: {}", sierra_json_path.display());
                continue; // Skip this file and proceed with the next one
            }
        };

        let contract_artifact: FlattenedSierraClass =
            match read_sierra_json_program(&sierra_json_file) {
                Ok(artifact) => artifact,
                Err(_) => {
                    println!("Error reading Sierra JSON program: {}", sierra_json_path.display());
                    continue; // Skip this file and proceed with the next one
                }
            };

        let filename = sierra_json_path.file_name().unwrap();
        contract_statistics.push(get_contract_statistics_for_file(
            filename.to_string_lossy().to_string(),
            sierra_json_file,
            contract_artifact,
        ));
    }
    contract_statistics
}

pub fn compare_against_limit(num1: u64, limit: u64) -> String {
    let warning_threshold = (limit as f64 * 0.8) as u64;
    let mut buffer = String::new();

    if num1 > limit {
        buffer.push_str("\x1b[0;31mDANGER\x1b[0m"); // Red color
        buffer.push_str(&format!("! You have passed the starknet limit of {}", limit));
    } else if num1 > warning_threshold {
        buffer.push_str("\x1b[0;33mWARNING\x1b[0m"); // Yellow color
        buffer.push_str(&format!("! You have reached 80% of the starknet limit of {}", limit));
    } else {
        buffer.push_str("\x1b[0;32mOK\x1b[0m"); // Green color
        buffer.push_str(" No warnings.");
    }

    buffer
}
