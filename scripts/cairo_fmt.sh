#!/bin/bash

option="--check"

if [ "$1" == "--fix" ]; then
    option=""
fi

scarb --manifest-path examples/spawn-and-move/Scarb.toml fmt $option
scarb --manifest-path examples/simple/Scarb.toml fmt $option
scarb --manifest-path crates/dojo/core/Scarb.toml fmt $option
scarb --manifest-path crates/dojo/core-cairo-test/Scarb.toml fmt $option
