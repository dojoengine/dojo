#!/bin/bash

# Can be run for one intergration test with: `--test TEST_NAME`

rm crates/benches/gas_usage.txt
cargo test bench $@ -- --ignored
cargo run --bin benches crates/benches/gas_usage.txt
