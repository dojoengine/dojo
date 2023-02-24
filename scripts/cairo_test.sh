#!/bin/bash

cargo +nightly-2022-11-03 run --bin dojo-test -- \
    --path lib

cargo +nightly-2022-11-03 run --bin dojo-test -- \
    --path examples
