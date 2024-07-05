#!/bin/bash

# When tests are run, the `build.rs` of `dojo-test-utils` is re-building the
# cairo artifacts ONLY if they don't exist.
# This script gives an easy way to remove those artifacts.

# A Katana instance must be running on http://localhost:8000.
# cargo run --bin katana

# Cleanup
rm -rf examples/spawn-and-move/target
rm -rf examples/spawn-and-move/manifests/dev/abis
rm -rf examples/spawn-and-move/manifests/dev/base
rm -rf examples/spawn-and-move/manifests/dev/manifest.json
rm -rf examples/spawn-and-move/manifests/dev/manifest.toml

rm -rf examples/spawn-and-move/manifests/release/abis
rm -rf examples/spawn-and-move/manifests/release/base
rm -rf examples/spawn-and-move/manifests/release/manifest.json
rm -rf examples/spawn-and-move/manifests/release/manifest.toml

rm -rf crates/torii/types-test/target
rm -rf crates/torii/types-test/manifests

cargo run --bin dojo-world-abigen

# Fix the cairo test to re-generate the code that is expected to be tested.
CAIRO_FIX_TESTS=1 cargo test --package dojo-lang plugin && \
CAIRO_FIX_TESTS=1 cargo test --package dojo-lang semantics

# Re-run the minimal tests, this will re-build the projects + generate the build artifacts.
cargo run -r --bin sozo -- build --manifest-path examples/spawn-and-move/Scarb.toml
cargo run -r --bin sozo -- build --manifest-path examples/spawn-and-move/Scarb.toml -P release
cargo run -r --bin sozo -- build --manifest-path crates/torii/types-test/Scarb.toml

# Finally, to include all the examples manifest, you should re-deploy the examples.
cargo run -r --bin sozo -- --offline migrate apply --manifest-path examples/spawn-and-move/Scarb.toml
