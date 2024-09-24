use katana_primitives::da::{blob, encoding, serde::parse_str_to_blob_data};
use num_bigint::BigUint;
use rstest::rstest;

fn read(path: &str) -> Vec<BigUint> {
    let content = std::fs::read_to_string(path).unwrap();
    let content = content.trim();
    parse_str_to_blob_data(content.strip_prefix("0x").unwrap_or(&content))
}

#[rstest]
#[case("./tests/test-data/blobs/blob1.txt")]
fn parse_blobs(#[case] blob: &str) {
    let encoded = blob::recover(read(blob));
    let state_update = encoding::decode_state_updates(&encoded);
    println!("{}", serde_json::to_string_pretty(&state_update).unwrap());
}
