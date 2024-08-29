#!/bin/bash

cargo +nightly-2024-08-29 fmt --check --all -- "$@"
