#!/usr/bin/env zsh

cargo run -r -p katana -- \
    -b 30000 \
    --rpc-url https://starknet-sepolia.g.alchemy.com/v2/tpJeT1O1qbkpSY9HmOUCjFqGtaeqz961 \
    --fork-block-number 61401
