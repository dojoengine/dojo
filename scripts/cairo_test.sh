#!/bin/bash
set -euxo pipefail

cargo +nightly-2022-11-03 run --bin dojo-test -- -p crates/dojo-core
# Uncomment once erc crate passes
# cargo +nightly-2022-11-03 run --bin dojo-test -- -p crates/dojo-erc
cargo +nightly-2022-11-03 run --bin dojo-test -- -p examples
