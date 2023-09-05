#!/bin/bash

# Make sure we are in script dir
cd "$(dirname "$0")"
ROOT=$(pwd)

rm -rf ./inc

# Build wasm pack and copy pkg files
cd ../wasm && wasm-pack build -t no-modules && cd $ROOT && cp -r ../wasm/pkg ./inc/wasm/

cargo run --bin sozo build --manifest-path ../../../examples/ecs/Scarb.toml && cp -r ../../../examples/ecs/target ./inc/target
