#!/bin/bash

scarb --manifest-path examples/spawn-and-move/Scarb.toml fmt --check
scarb --manifest-path examples/simple/Scarb.toml fmt --check
scarb --manifest-path crates/dojo/core/Scarb.toml fmt --check
scarb --manifest-path crates/dojo/core-cairo-test/Scarb.toml fmt --check
