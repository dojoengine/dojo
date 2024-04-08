use anyhow::Result;
use clap::Args;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::FlattenedSierraClass;
use std::fs::File;

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
    contract_artifact.sierra_program.iter().count()
}

pub fn get_file_size_in_bytes(file: File) -> u64 {
    file.metadata().unwrap().len()
}
