#!/bin/bash

cargo +nightly-2024-08-28 fmt --check --all -- "$@"
