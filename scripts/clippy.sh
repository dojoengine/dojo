#!/bin/bash

#!/bin/bash

run_clippy() {
  cargo clippy --all-targets --all-features "$@" -- -D warnings -D future-incompatible -D nonstandard-style -D rust-2018-idioms -D unused
}

run_clippy --manifest-path crates/torii/client/wasm/Cargo.toml --target wasm32-unknown-unknown && run_clippy

