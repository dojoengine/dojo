#!/bin/bash


option="--check"

if [ "$1" == "--fix" ]; then
    option=""
    shift
fi

cargo +nightly-2024-08-28 fmt $option --all -- "$@"
