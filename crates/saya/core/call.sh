#!/usr/bin/env bash

# Check if the argument is provided
if [ $# -eq 0 ]; then
    echo "Usage: $0 <calldata_file>"
    exit 1
fi

# Check if the file exists
if [ ! -f "$1" ]; then
    echo "Error: File '$1' not found."
    exit 1
fi

# Read calldata from the specified file
calldata=$(<$1)

# Pass the calldata to the sncast command
sncast --url https://free-rpc.nethermind.io/sepolia-juno/v0_6 \
    --account ts invoke \
    --contract-address 0x1b9c4e973ca9af0456eb6ae4c4576c5134905d8a560e0dfa1b977359e2c40ec \
    --function verify_and_register_fact \
    --calldata $calldata