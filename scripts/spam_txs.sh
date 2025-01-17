#!/usr/bin/env bash
#
# This scripts spams transactions to the spawn-and-move example by targeting
# the set_models function.
#
# Usage:
#   ./spam_txs.sh 100
#
# This will send 100 transactions to the spawn-and-move example.
#
# If working locally with Katana, use release and the `--dev` option to ensure faster setup:
# cargo run -r --bin katana -- --dev
#
# Uncomment to see the commands being executed.
# set -x
set -e

# Check if an argument is provided to display usage.
if [ $# -eq 0 ]; then
    echo "Usage: $0 <number_of_transactions> [rpc_url: http://0.0.0.0:5050]"
    echo "Example to send on local Katana: $0 100"
    exit 1
fi

# Number of transactions to send.
count="$1"

# RPC URL to use, default to local.
RPC_URL="${2:-http://0.0.0.0:5050}"

# Send transactions with random seeds to generate a bunch of entities.
for ((i=1; i<=count; i++))
do
    # Generates a random 248-bit number (to be sure it's fit in a felt252).
    seed=$(od -An -tx1 -N31 /dev/urandom | tr -d '  \n' | sed 's/^/0x/')
    # You can change seed by `$i` for reproducibility.
    seed=$i
    #seed=$(($i + 100000))

    # Number of models to spawn
    n_models=$((seed % 4 + 1))
    # You can set the number of models for reproducibility.
    n_models=1

    sozo execute actions set_models -c "$seed","$n_models" \
        --manifest-path examples/spawn-and-move/Scarb.toml \
        --rpc-url "$RPC_URL"

    #sleep 1
done
