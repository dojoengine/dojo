#!/bin/bash

# usage: bash scripts/build_cairo_projects.sh <sozo_path>
sozo_path=$1

# Re-run the minimal tests, this will re-build the projects + generate the build artifacts.
$sozo_path build --manifest-path examples/spawn-and-move/Scarb.toml
$sozo_path build --manifest-path examples/spawn-and-move/Scarb.toml -P release
$sozo_path build --manifest-path examples/simple/Scarb.toml
