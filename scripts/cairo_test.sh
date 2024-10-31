#!/bin/bash
set -euxo pipefail

cargo run -r --bin sozo -- --manifest-path crates/dojo/core/Scarb.toml test $@
cargo run -r --bin sozo -- --manifest-path examples/spawn-and-move/Scarb.toml test $@
