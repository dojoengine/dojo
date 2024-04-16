use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::PathBuf;

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::FlattenedSierraClass;

#[derive(Debug, PartialEq)]
pub struct ContractStatistics {
    pub contract_name: String,
    pub number_felts: u64,
    pub file_size: u64,
}

fn read_sierra_json_program(file: &File) -> Result<FlattenedSierraClass> {
    let contract_artifact: SierraClass = serde_json::from_reader(BufReader::new(file))?;
    let contract_artifact: FlattenedSierraClass = contract_artifact.flatten()?;

    Ok(contract_artifact)
}

fn get_sierra_byte_code_size(contract_artifact: FlattenedSierraClass) -> u64 {
    contract_artifact.sierra_program.len() as u64
}

fn get_file_size(file: &File) -> Result<u64, io::Error> {
    file.metadata().map(|metadata| metadata.len())
}

fn get_contract_statistics_for_file(
    contract_name: String,
    sierra_json_file: File,
    contract_artifact: FlattenedSierraClass,
) -> Result<ContractStatistics> {
    let file_size = get_file_size(&sierra_json_file).context(format!("Error getting file size"))?;
    let number_felts = get_sierra_byte_code_size(contract_artifact);
    Ok(ContractStatistics { file_size, contract_name, number_felts })
}

pub fn get_contract_statistics_for_dir(
    target_directory: &Utf8PathBuf,
) -> Result<Vec<ContractStatistics>> {
    let mut contract_statistics = Vec::new();
    let target_directory = target_directory.as_str();
    let dir: fs::ReadDir = fs::read_dir(target_directory)?;
    for entry in dir {
        let path: PathBuf = entry?.path();

        if path.is_dir() {
            continue;
        }

        let contract_name: String =
            path.file_stem().context("Error getting file name")?.to_string_lossy().to_string();

        let sierra_json_file: File =
            File::open(&path).context(format!("Error opening file: {}", path.to_string_lossy()))?;

        let contract_artifact: FlattenedSierraClass = read_sierra_json_program(&sierra_json_file)
            .context(format!(
            "Error parsing Sierra class artifact: {}",
            path.to_string_lossy()
        ))?;

        contract_statistics.push(get_contract_statistics_for_file(
            contract_name,
            sierra_json_file,
            contract_artifact,
        )?);
    }
    Ok(contract_statistics)
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::path::Path;

    use camino::Utf8PathBuf;

    use super::{
        get_contract_statistics_for_dir, get_contract_statistics_for_file, get_file_size,
        get_sierra_byte_code_size, read_sierra_json_program, ContractStatistics,
    };

    const TEST_SIERRA_JSON_CONTRACT: &str =
        "../../../bin/sozo/tests/test_data/compiled_contracts/test_contract.json";
    const TEST_SIERRA_FOLDER_CONTRACTS: &str =
        "../../../bin/sozo/tests/test_data/compiled_contracts/";

    #[test]
    fn get_sierra_byte_code_size_returns_correct_size() {
        let sierra_json_file = File::open(TEST_SIERRA_JSON_CONTRACT)
            .unwrap_or_else(|err| panic!("Failed to open file: {}", err));
        let flattened_sierra_class = read_sierra_json_program(&sierra_json_file)
            .unwrap_or_else(|err| panic!("Failed to read JSON program: {}", err));
        const EXPECTED_NUMBER_OF_FELTS: u64 = 2175;

        let number_of_felts = get_sierra_byte_code_size(flattened_sierra_class);

        assert_eq!(
            number_of_felts, EXPECTED_NUMBER_OF_FELTS,
            "Number of felts mismatch. Expected {}, got {}",
            EXPECTED_NUMBER_OF_FELTS, number_of_felts
        );
    }

    #[test]
    fn get_contract_statistics_for_file_returns_correct_statistics() {
        let sierra_json_file = File::open(TEST_SIERRA_JSON_CONTRACT)
            .unwrap_or_else(|err| panic!("Failed to open file: {}", err));
        let contract_artifact = read_sierra_json_program(&sierra_json_file)
            .unwrap_or_else(|err| panic!("Failed to read JSON program: {}", err));
        let filename = Path::new(TEST_SIERRA_JSON_CONTRACT)
            .file_stem()
            .expect("Error getting file name")
            .to_string_lossy()
            .to_string();
        let expected_contract_statistics: ContractStatistics = ContractStatistics {
            contract_name: String::from("test_contract"),
            number_felts: 2175,
            file_size: 114925,
        };

        let statistics =
            get_contract_statistics_for_file(filename.clone(), sierra_json_file, contract_artifact)
                .expect("Error getting contract statistics for file");

        assert_eq!(statistics, expected_contract_statistics);
    }

    #[test]
    fn get_contract_statistics_for_dir_returns_correct_statistics() {
        let target_dir = Utf8PathBuf::from(TEST_SIERRA_FOLDER_CONTRACTS);

        let contract_statistics = get_contract_statistics_for_dir(&target_dir)
            .expect(format!("Error getting contracts in dir {target_dir}").as_str());

        assert_eq!(contract_statistics.len(), 1, "Mismatch number of contract statistics");
    }

    #[test]
    fn get_file_size_returns_correct_size() {
        let sierra_json_file = File::open(TEST_SIERRA_JSON_CONTRACT)
            .unwrap_or_else(|err| panic!("Failed to open test file: {}", err));
        const EXPECTED_SIZE: u64 = 114925;

        let file_size = get_file_size(&sierra_json_file)
            .expect(format!("Error getting file size for test file").as_str());

        assert_eq!(file_size, EXPECTED_SIZE, "File size mismatch");
    }

    #[test]
    fn read_sierra_json_program_returns_ok_when_successful() {
        // Arrange
        let sierra_json_file = File::open(TEST_SIERRA_JSON_CONTRACT)
            .unwrap_or_else(|err| panic!("Failed to open test file: {}", err));

        let result = read_sierra_json_program(&sierra_json_file);

        assert!(result.is_ok(), "Expected Ok result");
    }
}
