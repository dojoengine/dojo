#!/bin/bash

CMD=starkli

ACCOUNT=katana-0
CLASS_FILE=target/dev/hello_hello.contract_class.json
RPC=http://localhost:5050

if ! command -v ${CMD} 2>&1 >/dev/null
then
    echo "${CMD} could not be found"
    exit 1
fi

CLASSHASH=$(${CMD} declare --account ${ACCOUNT} ${TARGET_FILE} --rpc ${RPC} ${CLASS_FILE} 2> /dev/null | tail -1)
echo "Class declared: ${CLASSHASH}"

ADDRESS=$(${CMD} deploy --account ${ACCOUNT} --rpc ${RPC} ${CLASSHASH} 2> /dev/null | tail -1)
echo "Contract deployed: ${ADDRESS}"

echo ""
echo "Add this to your dojo_dev.toml file:"
echo "[[external_contracts]]"
echo contract_name = \"Hello\"
echo contract_address = \"${ADDRESS}\"
