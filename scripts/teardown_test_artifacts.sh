#!/bin/bash

# When tests are run, the `build.rs` of `dojo-test-utils` is re-building the
# cairo artifacts ONLY if they don't exist.
# This script gives an easy way to remove those artifacts.

rm -rf examples/spawn-and-move/target
rm -rf examples/spawn-and-move/manifests

rm -rf crates/torii/types-test/target
rm -rf crates/torii/types-test/manifests
