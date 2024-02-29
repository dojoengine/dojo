#!/bin/bash

RUSTDOCFLAGS="-Dwarnings" cargo doc --document-private-items --no-deps --all-features --workspace --exclude katana
RUSTDOCFLAGS="-Dwarnings" cargo doc --document-private-items --no-deps -p katana
