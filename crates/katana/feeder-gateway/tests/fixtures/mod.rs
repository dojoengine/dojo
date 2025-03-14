#![allow(dead_code)]

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use katana_feeder_gateway::client::SequencerGateway;
use serde::de::DeserializeOwned;

/// Load test data from a JSON file located in the `/tests/fixtures` directory.
pub fn test_data<T: DeserializeOwned>(path: &str) -> T {
    let dir = PathBuf::from("tests/fixtures");
    let file = File::open(dir.join(path)).expect("failed to open file");
    serde_json::from_reader(BufReader::new(file)).expect("failed to read file")
}

#[rstest::fixture]
pub fn gateway() -> SequencerGateway {
    SequencerGateway::sn_mainnet()
}

#[rstest::fixture]
pub fn gateway_sepolia() -> SequencerGateway {
    SequencerGateway::sn_mainnet()
}
