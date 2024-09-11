#!/bin/bash

if [[ "$1" == "--fix" ]];
then
    ARGS=""
    shift
else
    ARGS="--check"
fi

cargo +nightly-2024-08-28 fmt ${ARGS} --all -- "$@"
