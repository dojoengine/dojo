#!/bin/bash

run_clippy() {
  cargo clippy --all-targets "$@" -- -D warnings -D future-incompatible -D nonstandard-style -D rust-2018-idioms -D unused
}

run_clippy --all-features --workspace --exclude katana --exclude katana-executor

run_clippy -p katana-executor --all
run_clippy -p katana
run_clippy -p katana --no-default-features --features sir
