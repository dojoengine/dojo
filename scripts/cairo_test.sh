#!/bin/bash
set -euxo pipefail

cargo +nightly-2022-11-03 run --bin dojo-test -- -p lib
cargo +nightly-2022-11-03 run --bin dojo-test -- -p examples
