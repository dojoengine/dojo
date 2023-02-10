#!/bin/bash

cargo +nightly-2022-11-03 run --manifest-path ./cairo/Cargo.toml --bin cairo-test -- \
    --path lib --starknet
