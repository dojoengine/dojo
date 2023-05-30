#!/bin/bash
set -euxo pipefail

cargo +nightly-2023-05-28 run --bin dojo-test -- crates/dojo-core
cargo +nightly-2023-05-28 run --bin dojo-test -- crates/dojo-erc
#cargo +nightly-2023-05-28 run --bin dojo-test -- crates/dojo-physics
cargo +nightly-2023-05-28 run --bin dojo-test -- crates/dojo-core/tests
# cargo +nightly-2023-05-28 run --bin dojo-test -- crates/dojo-defi
cargo +nightly-2023-05-28 run --bin dojo-test -- examples
