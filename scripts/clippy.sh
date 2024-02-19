#!/bin/bash

run_clippy() {
  cargo clippy --all-targets "$@" -- -D warnings -D future-incompatible -D nonstandard-style -D rust-2018-idioms -D unused
}

run_clippy --all-features --workspace --exclude katana && run_clippy -p katana
