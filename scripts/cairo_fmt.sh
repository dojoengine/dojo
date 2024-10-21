#!/bin/bash

scarb --manifest-path examples/spawn-and-move/Scarb.toml fmt --check
scarb --manifest-path crates/dojo/core/Scarb.toml fmt --check
