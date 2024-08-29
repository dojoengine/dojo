#!/bin/bash

cargo +nightly-2024-08-26 fmt --check --all -- "$@"
