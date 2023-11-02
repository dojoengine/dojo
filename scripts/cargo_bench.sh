#!/bin/bash

rm crates/benches/gas_usage.txt
cargo test bench -- --ignored
cargo run --bin benches crates/benches/gas_usage.txt
