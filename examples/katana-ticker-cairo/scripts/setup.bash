#!/bin/bash

EXE_DIR=$(dirname $(readlink -f ${0}))
TOP_DIR=$(dirname ${EXE_DIR})

OPERATOR="--account ${TOP_DIR}/config/operator.json --private-key 0x0300001800000000300000180000000000030000000000003006001800006600"

CONTRACT="${TOP_DIR}/contract/target/dev/counter_target_CounterTarget.sierra.json"


die() {
    echo "ERROR: $*"
    exit 42
}

starkli_cmd() {
    STARKNET_RPC="http://localhost:5050" starkli $* 2>&1 | grep -v "WARNING: using private key in plain text is highly insecure"
}

declare_counter() {
    local tmpfile=$(mktemp)
    starkli_cmd declare ${OPERATOR} ${CONTRACT} | tee ${tmpfile}
    sleep 3
    COUNTER_HASH="$(tail -n 1 ${tmpfile} | sed 's/\n//g')"
}

deploy_counter() {
    local tmpfile=$(mktemp)
    starkli_cmd deploy ${OPERATOR} ${COUNTER_HASH} 0x071c | tee ${tmpfile}
    sleep 3
    COUNTER_ADDR="$(tail -n 1 ${tmpfile} | sed 's/\n//g')"
}

setup_target() {
    starkli_cmd invoke --watch ${OPERATOR} 0x71c set_target ${COUNTER_ADDR}
    sleep 2
}

get_target_addr() {
    local output=$(starkli_cmd storage 0x71c $(starkli selector target))
    TICKER_TGT=${output}
}

echo "### Declare counter"
declare_counter
echo
echo "### Deploy counter"
deploy_counter
echo
echo "### Setup target"
setup_target

get_target_addr

echo
echo
echo "###################################################"
echo "Counter hash: ${COUNTER_HASH}"
echo "Counter address: ${COUNTER_ADDR}"
echo "Ticker target: ${TICKER_TGT}"