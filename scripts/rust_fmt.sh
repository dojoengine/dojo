#!/bin/bash

cargo +nightly-2024-08-24 fmt --check --all -- "$@"
