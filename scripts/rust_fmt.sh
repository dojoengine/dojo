#!/bin/bash

cargo +nightly-2024-08-25 fmt --check --all -- "$@"
