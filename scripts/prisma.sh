#!/bin/bash

cargo run --manifest-path ./crates/dojo-indexer/Cargo.toml --bin prisma-cli -- "$@"