#!/bin/bash

cargo +nightly-2024-08-27 fmt --check --all -- "$@"
