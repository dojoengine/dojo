#!/bin/bash
set -euxo pipefail

cargo +nightly-2022-11-03 run --bin dojo-test -- crates/dojo-core
# Uncomment once erc crate passes
# cargo +nightly-2022-11-03 run --bin dojo-test -- crates/dojo-erc
# cargo +nightly-2022-11-03 run --bin dojo-test -- crates/dojo-physics
cargo +nightly-2022-11-03 run --bin dojo-test -- examples
