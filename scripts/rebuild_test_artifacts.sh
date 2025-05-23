#!/bin/bash

# When tests are run, the `build.rs` of `dojo-test-utils` is re-building the
# cairo artifacts ONLY if they don't exist.
# This script gives an easy way to remove those artifacts.

cargo build -r --bin sozo

# Some formatting.
cargo +nightly-2024-08-28 fmt --all -- "$@"

scarb --manifest-path examples/spawn-and-move/Scarb.toml fmt
scarb --manifest-path examples/simple/Scarb.toml fmt
scarb --manifest-path crates/dojo/core/Scarb.toml fmt
scarb --manifest-path crates/dojo/core-test/Scarb.toml fmt
scarb --manifest-path crates/dojo/dojo-cairo-test/Scarb.toml fmt
scarb --manifest-path crates/dojo/dojo-snf-test/Scarb.toml fmt

# Manual forced cleanup.
rm -rf examples/spawn-and-move/target

# Ensure the world bindings are up to date.
cargo run --bin dojo-world-abigen -r

cargo +nightly-2024-08-28 fmt --all -- "$@"

# Re-run the minimal tests, this will re-build the projects + generate the build artifacts.
./target/release/sozo build --manifest-path examples/simple/Scarb.toml
./target/release/sozo build --manifest-path examples/spawn-and-move/Scarb.toml
./target/release/sozo build --manifest-path examples/spawn-and-move/Scarb.toml -P release
./target/release/sozo test --manifest-path crates/dojo/core-tests/Scarb.toml

# Generates the database for testing by migrating the spawn and move example.
KATANA_RUNNER_BIN=/tmp/katana cargo generate-test-db

# Extracts the database for testing.
bash ./scripts/extract_test_db.sh

