use anyhow::Result;
use katana_primitives::da::eip4844::BLOB_LEN;
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
    // the fft'd version
    let fftd = read(blob);
    // the ifft'd version
    let encoded = blob::recover(fftd.clone());

    let state_update = encoding::decode_state_updates(&encoded)?;
    let mut reencoded = encode_state_updates(state_update);

    // TODO: put this directly in the encoding module
    while reencoded.len() < BLOB_LEN {
        reencoded.push(BigUint::ZERO);
    }

    // re-fft the reencoded data
    let refftd = blob::transform(reencoded.clone());

    // assert that our encoding and transformation functions are correct
    similar_asserts::assert_eq!(encoded, reencoded);
    similar_asserts::assert_eq!(fftd, refftd);

    Ok(())
}
