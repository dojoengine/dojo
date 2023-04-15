#!/bin/bash
set -euxo pipefail

cargo +nightly-2022-11-03 run --bin dojo-test -- -p crates/dojo-core
cargo +nightly-2022-11-03 run --bin dojo-test -- -p crates/dojo-erc
cargo +nightly-2022-11-03 run --bin dojo-test -- -p crates/dojo-physics
cargo +nightly-2022-11-03 run --bin dojo-test -- -p examples
