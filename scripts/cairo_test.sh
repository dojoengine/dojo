#!/bin/bash
set -euxo pipefail

cargo +nightly-2023-05-28 run --bin sozo -- --manifest-path crates/dojo-core/Scarb.toml test
# cargo +nightly-2023-05-28 run --bin sozo -- --manifest-path crates/dojo-erc/Scarb.toml test
#cargo +nightly-2023-05-28 run --bin sozo -- test crates/dojo-physics
cargo +nightly-2023-05-28 run --bin sozo -- --manifest-path crates/dojo-core/tests/Scarb.toml test
# cargo +nightly-2023-05-28 run --bin sozo -- test crates/dojo-defi
cargo +nightly-2023-05-28 run --bin sozo -- --manifest-path examples/ecs/Scarb.toml test
