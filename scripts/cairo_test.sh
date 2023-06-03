#!/bin/bash
set -euxo pipefail

cargo +nightly-2023-05-28 run --bin sozo -- test crates/dojo-core
cargo +nightly-2023-05-28 run --bin sozo -- test crates/dojo-erc
#cargo +nightly-2023-05-28 run --bin sozo -- test crates/dojo-physics
cargo +nightly-2023-05-28 run --bin sozo -- test crates/dojo-core/tests
# cargo +nightly-2023-05-28 run --bin sozo -- test crates/dojo-defi
cargo +nightly-2023-05-28 run --bin sozo -- test examples/ecs
