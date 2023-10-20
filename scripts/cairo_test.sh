#!/bin/bash
set -euxo pipefail

cargo run -r --bin sozo -- --manifest-path crates/dojo-core/Scarb.toml test $@
# cargo run --bin sozo -- test crates/dojo-physics
cargo run -r --bin sozo -- --manifest-path crates/dojo-erc/Scarb.toml test $@
cargo run -r --bin sozo -- --manifest-path crates/dojo-defi/Scarb.toml test $@
cargo run -r --bin sozo -- --manifest-path crates/dojo-primitives/Scarb.toml test $@
cargo run -r --bin sozo -- --manifest-path examples/spawn-and-move/Scarb.toml test $@
