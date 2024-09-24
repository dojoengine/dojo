use anyhow::Result;
use katana_primitives::da::encoding::encode_state_updates;
use katana_primitives::da::serde::parse_str_to_blob_data;
use katana_primitives::da::{blob, encoding};
use num_bigint::BigUint;
use rstest::rstest;

fn read(path: &str) -> Vec<BigUint> {
    let content = std::fs::read_to_string(path).unwrap();
    let content = content.trim();
    parse_str_to_blob_data(content.strip_prefix("0x").unwrap_or(content))
}

/// Pre-SNAR Tree blobs
#[rstest]
#[case("./tests/test-data/blobs/block_636262.txt")]
#[case("./tests/test-data/blobs/block_636263.txt")]
#[case("./tests/test-data/blobs/block_636264.txt")]
fn parse_blobs_rt(#[case] blob: &str) -> Result<()> {
    let encoded = blob::recover(read(blob));
    let state_update = encoding::decode_state_updates(&encoded)?;
    let _ = encode_state_updates(state_update);
    Ok(())
}
