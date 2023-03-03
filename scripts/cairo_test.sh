#!/bin/bash
set -euxo pipefail

cargo +nightly-2022-11-03 run --bin dojo-test -- \
    -p lib/src

cargo +nightly-2022-11-03 run --bin dojo-test -- \
    --path examples/src
