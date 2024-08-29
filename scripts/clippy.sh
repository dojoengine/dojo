#!/bin/bash

# Tells the shell to exit immediately if a command exits with a non-zero status
set -e
# Enables tracing of the commands as they are executed, showing the commands and their arguments
set -x
# Causes a pipeline to return a failure status if any command in the pipeline fails
set -o pipefail

run_clippy() {
  cargo +nightly-2024-08-28 clippy --all-targets "$@" -- -D warnings -D future-incompatible -D nonstandard-style -D rust-2018-idioms -D unused -D missing-debug-implementations
}

run_clippy --all-features --workspace
