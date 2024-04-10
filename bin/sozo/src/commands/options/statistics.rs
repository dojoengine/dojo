use std::fs::{self, File};
use std::path::PathBuf;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::FlattenedSierraClass;

#[derive(Debug, PartialEq)]
pub struct ContractStatistics {
    pub contract_name: String,
    pub number_felts: u64,
    pub file_size: u64,
}

#[derive(Debug, Args)]
pub struct Stats {
    #[arg(long, help = "Display statistics")]
    pub stats: bool,
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

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::path::Path;

    use camino::Utf8PathBuf;

    use super::{
        compute_contract_byte_code_size, get_contract_statistics_for_dir,
        get_contract_statistics_for_file, get_file_size_in_bytes, read_sierra_json_program,
        ContractStatistics,
    };

    const TEST_SIERRA_JSON_CONTRACT: &str =
        "tests/test_data/sierra_compiled_contracts/contracts_test.contract_class.json";
    const TEST_SIERRA_FOLDER_CONTRACTS: &str = "tests/test_data/sierra_compiled_contracts/";

    #[test]
    fn compute_contract_byte_code_size_returns_correct_size() {
        // Arrange
        let sierra_json_file = File::open(TEST_SIERRA_JSON_CONTRACT)
            .unwrap_or_else(|err| panic!("Failed to open file: {}", err));
        let flattened_sierra_class = read_sierra_json_program(&sierra_json_file)
            .unwrap_or_else(|err| panic!("Failed to read JSON program: {}", err));
        let expected_number_of_felts: usize = 448;

        // Act
        let number_of_felts = compute_contract_byte_code_size(flattened_sierra_class);

        // Assert
        assert_eq!(
            number_of_felts, expected_number_of_felts,
            "Number of felts mismatch. Expected {}, got {}",
            expected_number_of_felts, number_of_felts
        );
    }

    #[test]
    fn get_contract_statistics_for_file_returns_correct_statistics() {
        // Arrange
        let sierra_json_file = File::open(TEST_SIERRA_JSON_CONTRACT)
            .unwrap_or_else(|err| panic!("Failed to open file: {}", err));
        let contract_artifact = read_sierra_json_program(&sierra_json_file)
            .unwrap_or_else(|err| panic!("Failed to read JSON program: {}", err));
        let filename =
            Path::new(TEST_SIERRA_JSON_CONTRACT).file_name().unwrap().to_string_lossy().to_string();
        let expected_contract_statistics: ContractStatistics = ContractStatistics {
            contract_name: String::from("contracts_test.contract_class.json"),
            number_felts: 448,
            file_size: 38384,
        };

        // Act
        let statistics =
            get_contract_statistics_for_file(filename.clone(), sierra_json_file, contract_artifact);

        // Assert
        assert_eq!(statistics, expected_contract_statistics);
    }

    #[test]
    fn get_contract_statistics_for_dir_returns_correct_statistics() {
        // Arrange
        let path_full_of_built_sierra_contracts = Utf8PathBuf::from(TEST_SIERRA_FOLDER_CONTRACTS);

        // Act
        let contract_statistics =
            get_contract_statistics_for_dir(&path_full_of_built_sierra_contracts);

        // Assert
        assert_eq!(contract_statistics.len(), 1, "Mismatch number of contract statistics");
    }

    #[test]
    fn get_file_size_in_bytes_returns_correct_size() {
        // Arrange
        let sierra_json_file = File::open(TEST_SIERRA_JSON_CONTRACT)
            .unwrap_or_else(|err| panic!("Failed to open file: {}", err));
        const EXPECTED_SIZE: u64 = 38384;

        // Act
        let file_size = get_file_size_in_bytes(sierra_json_file);

        // Assert
        assert_eq!(file_size, EXPECTED_SIZE, "File size mismatch");
    }

    #[test]
    fn read_sierra_json_program_returns_ok_when_successful() {
        // Arrange
        let sierra_json_file = File::open(TEST_SIERRA_JSON_CONTRACT)
            .unwrap_or_else(|err| panic!("Failed to open file: {}", err));

        // Act
        let result = read_sierra_json_program(&sierra_json_file);

        // Assert
        assert!(result.is_ok(), "Expected Ok result");
    }
}
