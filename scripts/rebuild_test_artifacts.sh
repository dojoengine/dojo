#!/bin/bash

# When tests are run, the `build.rs` of `dojo-test-utils` is re-building the
# cairo artifacts ONLY if they don't exist.
# This script gives an easy way to remove those artifacts.

cargo build -r --bin katana

# some formatting:
cargo +nightly-2024-08-28 fmt --all -- "$@"

scarb --manifest-path examples/spawn-and-move/Scarb.toml fmt
scarb --manifest-path examples/simple/Scarb.toml fmt
scarb --manifest-path crates/dojo/core/Scarb.toml fmt
scarb --manifest-path crates/dojo/core-cairo-test/Scarb.toml fmt

cargo build -r --bin sozo

# Cleanup
rm -rf examples/spawn-and-move/target
rm -rf crates/torii/types-test/target
rm -rf crates/dojo/lang/src/manifest_test_data/compiler_cairo/target

# Ensure the world bindings are up to date.
cargo run --bin dojo-world-abigen -r

cargo +nightly-2024-08-28 fmt --all -- "$@"

# Fix the cairo test to re-generate the code that is expected to be tested.
# CAIRO_FIX_TESTS=1 cargo test --package dojo-lang plugin && \
# CAIRO_FIX_TESTS=1 cargo test --package dojo-lang semantics

# Re-run the minimal tests, this will re-build the projects + generate the build artifacts.
./target/release/sozo build --manifest-path examples/spawn-and-move/Scarb.toml
./target/release/sozo build --manifest-path examples/spawn-and-move/Scarb.toml -P release
./target/release/sozo build --manifest-path crates/torii/types-test/Scarb.toml

# Generates the database for testing by migrating the spawn and move example.
KATANA_RUNNER_BIN=./target/release/katana cargo generate-test-db
# Ensure the user has locally the db dir in /tmp.
rm -rf /tmp/spawn-and-move-db
rm -rf /tmp/types-test-db
tar xzf spawn-and-move-db.tar.gz -C /tmp/
tar xzf types-test-db.tar.gz -C /tmp/
