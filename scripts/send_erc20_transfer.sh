#!/bin/bash

if [ $# -eq 0 ]; then
    echo "Error: Contract address argument is required."
    echo "Usage: $0 <contract_address>"
    exit 1
fi

contract_address=$1
rpc="http://localhost:5050"

starkli invoke $contract_address transfer 0x1234 u256:1 --account ../account.json --keystore ../signer.json --keystore-password "" --rpc $rpc
