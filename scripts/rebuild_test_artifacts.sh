#!/bin/bash

# When tests are run, the `build.rs` of `dojo-test-utils` is re-building the
# cairo artifacts ONLY if they don't exist.
# This script gives an easy way to remove those artifacts.

cargo build -r --bin sozo

# Some formatting.
bash ./scripts/rust_fmt.sh --fix
bash ./scripts/cairo_fmt.sh fmt

# Manual forced cleanup.
rm -rf examples/spawn-and-move/target

# Ensure the world bindings are up to date.
cargo run --bin dojo-world-abigen -r

# Re-run the minimal tests, this will re-build the projects + generate the build artifacts.
./target/release/sozo build --manifest-path examples/simple/Scarb.toml
./target/release/sozo build --manifest-path examples/spawn-and-move/Scarb.toml
./target/release/sozo build --manifest-path examples/spawn-and-move/Scarb.toml -P release
./target/release/sozo test --manifest-path crates/dojo/core-tests/Scarb.toml

# Generates the database for testing by migrating the spawn and move example.
KATANA_RUNNER_BIN=/tmp/katana cargo generate-test-db

# Extracts the database for testing.
bash ./scripts/extract_test_db.sh
