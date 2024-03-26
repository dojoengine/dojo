#!/bin/bash

# Install Cargo Clippy for the specified toolchain
rustup component add clippy --toolchain 1.76.0-x86_64-unknown-linux-gnu

run_clippy() {
  cargo clippy --all-targets "$@" -- -D warnings -D future-incompatible -D nonstandard-style -D rust-2018-idioms -D unused
}

run_clippy --all-features --workspace --exclude katana --exclude katana-executor

run_clippy -p katana-executor --all
run_clippy -p katana
run_clippy -p katana --no-default-features --features sir
