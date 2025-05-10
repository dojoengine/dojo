#!/bin/bash

option="--check"

if [ "$1" == "--fix" ]; then
    option=""
fi

scarb --manifest-path crates/dojo/core/Scarb.toml fmt $option
scarb --manifest-path crates/dojo/macros/Scarb.toml fmt $option
scarb --manifest-path crates/dojo/core-tests/Scarb.toml fmt $option

scarb --manifest-path examples/simple/Scarb.toml fmt $option
scarb --manifest-path examples/spawn-and-move/Scarb.toml fmt $option
scarb --manifest-path examples/benchmark/Scarb.toml fmt $option
