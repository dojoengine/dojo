#!/bin/bash
set -euxo pipefail

cargo run --bin sozo -- --manifest-path crates/dojo-core/Scarb.toml test
# cargo run --bin sozo -- test crates/dojo-physics
cargo run --bin sozo -- --manifest-path crates/dojo-erc/Scarb.toml test
# cargo run --bin sozo -- test crates/dojo-defi
cargo run --bin sozo -- --manifest-path examples/ecs/Scarb.toml test
# cargo run --bin sozo -- test crates/dojo-chess
cargo run --bin sozo -- --manifest-path examples/dojo-chess/Scarb.toml test
