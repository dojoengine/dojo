#!/bin/bash

if [[ "$1" == "--fix" ]];
then
    ARGS=""
else
    ARGS="--check"
fi

scarb --manifest-path examples/spawn-and-move/Scarb.toml fmt ${ARGS}
scarb --manifest-path crates/dojo-core/Scarb.toml fmt ${ARGS}

