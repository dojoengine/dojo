use anyhow::Result;
use camino::Utf8PathBuf;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::FlattenedSierraClass;
use std::fs::{self, File};
use std::io;
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub struct ContractStatistics {
    pub contract_name: String,
    pub number_felts: u64,
    pub file_size: u64,
}

fn read_sierra_json_program(file: &File) -> Result<FlattenedSierraClass> {
    let contract_artifact: SierraClass = serde_json::from_reader(file)?;
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
    file_name: String,
    sierra_json_file: File,
    contract_artifact: FlattenedSierraClass,
) -> ContractStatistics {
    ContractStatistics {
        contract_name: file_name,
        number_felts: get_sierra_byte_code_size(contract_artifact),
        file_size: get_file_size(&sierra_json_file)
            .expect(format!("Error getting file size for file").as_str()),
    }
}

pub fn get_contract_statistics_for_dir(
    target_directory: &Utf8PathBuf,
) -> Result<Vec<ContractStatistics>> {
    let mut contract_statistics = Vec::new();
    let target_directory = target_directory.as_str();
    let built_contract_paths: fs::ReadDir = fs::read_dir(target_directory)
        .expect(format!("Error reading dir {target_directory}").as_str());
    for sierra_json_path in built_contract_paths {
        let sierra_json_path_buff: PathBuf =
            sierra_json_path.expect("Error getting buffer for file").path();

        let file_name: String = sierra_json_path_buff
            .file_stem()
            .expect("Error getting file name")
            .to_string_lossy()
            .to_string();

        let sierra_json_path_str =
            sierra_json_path_buff.into_os_string().into_string().expect("String is expected");

        let sierra_json_file: File = File::open(&sierra_json_path_str)
            .expect(format!("Error opening Sierra JSON file: {sierra_json_path_str}").as_str());

        let contract_artifact: FlattenedSierraClass = read_sierra_json_program(&sierra_json_file)
            .expect(format!("Error reading Sierra JSON program: {sierra_json_path_str}").as_str());

        contract_statistics.push(get_contract_statistics_for_file(
            file_name,
            sierra_json_file,
            contract_artifact,
        ));
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

    const TEST_SIERRA_JSON_CONTRACT: &str = "../../../bin/sozo/tests/test_data/\
                                             sierra_compiled_contracts/contracts_test.\
                                             contract_class.json";
    const TEST_SIERRA_FOLDER_CONTRACTS: &str =
        "../../../bin/sozo/tests/test_data/sierra_compiled_contracts/";

    #[test]
    fn get_sierra_byte_code_size_returns_correct_size() {
        // Arrange
        let sierra_json_file = File::open(TEST_SIERRA_JSON_CONTRACT)
            .unwrap_or_else(|err| panic!("Failed to open file: {}", err));
        let flattened_sierra_class = read_sierra_json_program(&sierra_json_file)
            .unwrap_or_else(|err| panic!("Failed to read JSON program: {}", err));
        let expected_number_of_felts: u64 = 448;

        // Act
        let number_of_felts = get_sierra_byte_code_size(flattened_sierra_class);

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
        let filename = Path::new(TEST_SIERRA_JSON_CONTRACT)
            .file_stem()
            .expect("Error getting file name")
            .to_string_lossy()
            .to_string();
        let expected_contract_statistics: ContractStatistics = ContractStatistics {
            contract_name: String::from("contracts_test.contract_class"),
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
            get_contract_statistics_for_dir(&path_full_of_built_sierra_contracts).expect(
                format!("Error getting contracts in dir {path_full_of_built_sierra_contracts}")
                    .as_str(),
            );

        // Assert
        assert_eq!(contract_statistics.len(), 1, "Mismatch number of contract statistics");
    }

    #[test]
    fn get_file_size_returns_correct_size() {
        // Arrange
        let sierra_json_file = File::open(TEST_SIERRA_JSON_CONTRACT)
            .unwrap_or_else(|err| panic!("Failed to open test file: {}", err));
        const EXPECTED_SIZE: u64 = 38384;

        // Act
        let file_size = get_file_size(&sierra_json_file)
            .expect(format!("Error getting file size for test file").as_str());

        // Assert
        assert_eq!(file_size, EXPECTED_SIZE, "File size mismatch");
    }

    #[test]
    fn read_sierra_json_program_returns_ok_when_successful() {
        // Arrange
        let sierra_json_file = File::open(TEST_SIERRA_JSON_CONTRACT)
            .unwrap_or_else(|err| panic!("Failed to open test file: {}", err));

        // Act
        let result = read_sierra_json_program(&sierra_json_file);

        // Assert
        assert!(result.is_ok(), "Expected Ok result");
    }
}
