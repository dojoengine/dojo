#!/bin/bash

cargo +nightly fmt --check --all -- "$@"
