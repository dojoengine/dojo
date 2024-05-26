use cairo_lang_starknet_classes::casm_contract_class::CasmContractClass;
use cairo_lang_starknet_classes::contract_class::ContractClass;
use scarb_ui::Ui;
use std::fs::{self, File};
use std::io::{self, BufReader, Seek, SeekFrom};
use std::path::PathBuf;

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::FlattenedSierraClass;

#[derive(Debug, PartialEq)]
pub struct ContractStatistics {
    pub contract_name: String,

    pub sierra_bytecode_size: u64,
    pub sierra_contract_class_size: u64,

    pub casm_bytecode_size: u64,
    pub casm_contract_class_size: u64,
}

fn read_sierra_json_program(file: &File) -> Result<(FlattenedSierraClass, CasmContractClass)> {
    let mut buf_reader = BufReader::new(file);
    let contract_artifact: SierraClass = serde_json::from_reader(&mut buf_reader)?;
    buf_reader.seek(SeekFrom::Start(0)).unwrap();

    let contract_class: ContractClass = serde_json::from_reader(&mut buf_reader)?;
    buf_reader.seek(SeekFrom::Start(0)).unwrap();

    let casm_contract_class: CasmContractClass =
        CasmContractClass::from_contract_class(contract_class, false, usize::MAX)?;

    let contract_artifact: FlattenedSierraClass = contract_artifact.flatten()?;

    Ok((contract_artifact, casm_contract_class))
}

fn get_sierra_byte_code_size(contract_artifact: FlattenedSierraClass) -> u64 {
    contract_artifact.sierra_program.len() as u64
}

fn get_casm_byte_code_size(contract_artifact: CasmContractClass) -> u64 {
    contract_artifact.bytecode.len() as u64
}

fn get_file_size(file: &File) -> Result<u64, io::Error> {
    file.metadata().map(|metadata| metadata.len())
}

fn get_contract_statistics_for_file(
    contract_name: String,
    sierra_json_file: File,
    sierra_class: FlattenedSierraClass,
    casm_class: CasmContractClass,
) -> Result<ContractStatistics> {
    let sierra_contract_class_size =
        get_file_size(&sierra_json_file).context("Error getting file size")?;
    let sierra_bytecode_size = get_sierra_byte_code_size(sierra_class);

    let casm_contract_class_size = serde_json::to_string(&casm_class)
        .context("should be valid json")
        .unwrap()
        .len()
        .try_into()
        .unwrap();
    let casm_bytecode_size = get_casm_byte_code_size(casm_class);

    Ok(ContractStatistics {
        contract_name,
        sierra_bytecode_size,
        sierra_contract_class_size,
        casm_bytecode_size,
        casm_contract_class_size,
    })
}

pub fn get_contract_statistics_for_dir(
    ui: Ui,
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

        // To ignore files like `contract.contract_class.json` or `contract.compiled_contract_class.json`
        if contract_name.contains('.') {
            continue;
        }

        let sierra_json_file: File =
            File::open(&path).context(format!("Error opening file: {}", path.to_string_lossy()))?;

        let (sierra_class, casm_class) = match read_sierra_json_program(&sierra_json_file) {
            Ok(s) => s,
            Err(e) => {
                ui.verbose(format!("Unable to process file: {:?}\nWith error: {e:?}", &path));
                // skip any file which cannot be processed properly since there can be other file types in target folder
                // for example casm contract class.
                continue;
            }
        };

        contract_statistics.push(get_contract_statistics_for_file(
            contract_name,
            sierra_json_file,
            sierra_class,
            casm_class,
        )?);
    }
    Ok(contract_statistics)
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::path::Path;

    use camino::Utf8PathBuf;
    use scarb_ui::Ui;

    use crate::statistics::get_casm_byte_code_size;

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
        let (flattened_sierra_class, casm_class) = read_sierra_json_program(&sierra_json_file)
            .unwrap_or_else(|err| panic!("Failed to read JSON program: {}", err));

        const SIERRA_EXPECTED_NUMBER_OF_FELTS: u64 = 2175;

        let sierra_bytecode_size = get_sierra_byte_code_size(flattened_sierra_class);
        let casm_bytecode_size = get_casm_byte_code_size(casm_class);

        const CASM_EXPECTED_NUMBER_OF_FELTS: u64 = 4412;

        assert_eq!(
            sierra_bytecode_size, SIERRA_EXPECTED_NUMBER_OF_FELTS,
            "[Sierra] Number of felts mismatch. Expected {}, got {}",
            SIERRA_EXPECTED_NUMBER_OF_FELTS, sierra_bytecode_size
        );

        assert_eq!(
            casm_bytecode_size, CASM_EXPECTED_NUMBER_OF_FELTS,
            "[Casm] Number of felts mismatch. Expected {}, got {}",
            CASM_EXPECTED_NUMBER_OF_FELTS, casm_bytecode_size
        );
    }

    #[test]
    fn get_contract_statistics_for_file_returns_correct_statistics() {
        let sierra_json_file = File::open(TEST_SIERRA_JSON_CONTRACT)
            .unwrap_or_else(|err| panic!("Failed to open file: {}", err));

        let (sierra_class, casm_class) = read_sierra_json_program(&sierra_json_file)
            .unwrap_or_else(|err| panic!("Failed to read JSON program: {}", err));

        let filename = Path::new(TEST_SIERRA_JSON_CONTRACT)
            .file_stem()
            .expect("Error getting file name")
            .to_string_lossy()
            .to_string();

        let expected_contract_statistics: ContractStatistics = ContractStatistics {
            contract_name: String::from("test_contract"),
            sierra_bytecode_size: 2175,
            sierra_contract_class_size: 114925,

            casm_bytecode_size: 4412,
            casm_contract_class_size: 95806,
        };

        let statistics = get_contract_statistics_for_file(
            filename.clone(),
            sierra_json_file,
            sierra_class,
            casm_class,
        )
        .expect("Error getting contract statistics for file");

        assert_eq!(statistics, expected_contract_statistics);
    }

    #[test]
    fn get_contract_statistics_for_dir_returns_correct_statistics() {
        let target_dir = Utf8PathBuf::from(TEST_SIERRA_FOLDER_CONTRACTS);
        let ui = Ui::new(scarb_ui::Verbosity::Normal, scarb_ui::OutputFormat::Text);

        let contract_statistics = get_contract_statistics_for_dir(ui, &target_dir)
            .unwrap_or_else(|_| panic!("Error getting contracts in dir {target_dir}"));

        assert_eq!(contract_statistics.len(), 1, "Mismatch number of contract statistics");
    }

    #[test]
    fn get_file_size_returns_correct_size() {
        let sierra_json_file = File::open(TEST_SIERRA_JSON_CONTRACT)
            .unwrap_or_else(|err| panic!("Failed to open test file: {}", err));
        const EXPECTED_SIZE: u64 = 114925;

        let file_size = get_file_size(&sierra_json_file)
            .unwrap_or_else(|_| panic!("Error getting file size for test file"));

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
