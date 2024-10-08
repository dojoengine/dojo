#!/bin/bash

# set -a && source .env && set +as

# Set private variables

SAYA_SEPOLIA_ENDPOINT=https://api.cartridge.gg/x/starknet/sepolia
SAYA_SEPOLIA_PRIVATE_KEY=
SAYA_SEPOLIA_ACCOUNT_ADDRESS=
SAYA_PROVER_KEY=
SAYA_SNCAST_ACCOUNT_NAME="dev"

# Probably no need to change these

# SAYA_PROVER_URL=http://prover.visoft.dev:3618
# SAYA_PROVER_URL=http://localhost:3040
SAYA_PROVER_URL=https://api.cartridge.gg/prover/
# SAYA_MANIFEST_PATH=../shard-dungeon/Scarb.toml
SAYA_MANIFEST_PATH=examples/spawn-and-move/Scarb.toml
SAYA_FACT_REGISTRY_CLASS_HASH=0x0485857a88cacd0a706452c61cfa613802c638dc4ce09bf3d8b289c70183d293
SAYA_PILTOVER_CLASS_HASH=0x5b4d0cbbf0104781dd1c556aa4651f60edc4015d8cb4e8882bb46549bb46983
SAYA_PILTOVER_STARTING_STATE_ROOT=0
SAYA_CONFIG_HASH=42
SAYA_PROGRAM_HASH=0x2aa9e430c145b26d681a8087819ed5bff93f5596105d0e74f00fc7caa46fa18 #need to be reupdated

# Set after runnig the script

SAYA_WORLD_ADDRESS=""
SAYA_WORLD_PREPARED="" # Set to anything after preparing the world successfully for the first time
SAYA_FORK_BLOCK_NUMBER=
SAYA_SKIP_MAKING_TRANSACTIONS="" # Set to anything to skip making transactions
SAYA_PILTOVER_ADDRESS=""
SAYA_PILTOVER_PREPARED=
SAYA_FACT_REGISTRY=""


if [[ -z "${SAYA_FACT_REGISTRY}" ]]; then
    sncast -a $SAYA_SNCAST_ACCOUNT_NAME deploy \
        --class-hash $SAYA_FACT_REGISTRY_CLASS_HASH \
        --fee-token eth \
        -u $SAYA_SEPOLIA_ENDPOINT
    echo "Set SAYA_FACT_REGISTRY to the address of the deployed contract."
    exit 0
fi
if [[ -z "${SAYA_WORLD_ADDRESS}" ]]; then
  echo "World address not set: DEPLOYING WORLD"

    # Build world contract
    cargo run -r --bin sozo -- \
        build \
        --manifest-path $SAYA_MANIFEST_PATH

    cargo run -r --bin sozo -- \
        migrate apply \
        --manifest-path $SAYA_MANIFEST_PATH \
        --rpc-url $SAYA_SEPOLIA_ENDPOINT \
        --private-key $SAYA_SEPOLIA_PRIVATE_KEY \
        --account-address $SAYA_SEPOLIA_ACCOUNT_ADDRESS

    echo "Set SAYA_WORLD_ADDRESS to the address of the deployed contract."

    exit 0

else
  echo "Using world: $SAYA_WORLD_ADDRESS"
fi

if [[ -z "${SAYA_WORLD_PREPARED}" ]]; then
    echo "World not prepared: PREPARING WORLD"

    cargo run -r --bin sozo -- \
        execute $SAYA_WORLD_ADDRESS set_differ_program_hash \
        -c 2265722951651489608338464389196546125983429710081933755514038580032192121109 \
        --manifest-path $SAYA_MANIFEST_PATH \
        --rpc-url $SAYA_SEPOLIA_ENDPOINT \
        --private-key $SAYA_SEPOLIA_PRIVATE_KEY \
        --account-address $SAYA_SEPOLIA_ACCOUNT_ADDRESS \
        --fee-estimate-multiplier 20 \
        --world $SAYA_WORLD_ADDRESS \
        --wait

    cargo run -r --bin sozo -- \
        execute $SAYA_WORLD_ADDRESS set_merger_program_hash \
        -c 2265722951651489608338464389196546125983429710081933755514038580032192121109 \
        --manifest-path $SAYA_MANIFEST_PATH \
        --rpc-url $SAYA_SEPOLIA_ENDPOINT \
        --private-key $SAYA_SEPOLIA_PRIVATE_KEY \
        --account-address $SAYA_SEPOLIA_ACCOUNT_ADDRESS \
        --fee-estimate-multiplier 20 \
        --world $SAYA_WORLD_ADDRESS \
        --wait

    cargo run -r --bin sozo -- \
        execute $SAYA_WORLD_ADDRESS set_facts_registry \
        -c $SAYA_FACT_REGISTRY \
        --manifest-path $SAYA_MANIFEST_PATH \
        --rpc-url $SAYA_SEPOLIA_ENDPOINT \
        --private-key $SAYA_SEPOLIA_PRIVATE_KEY \
        --account-address $SAYA_SEPOLIA_ACCOUNT_ADDRESS \
        --fee-estimate-multiplier 20 \
        --world $SAYA_WORLD_ADDRESS \
        --wait

    echo "Set SAYA_WORLD_PREPARED to anything to skip this step next time."

else
  echo "World is already prepared"
fi

if [[ -z "${SAYA_FORK_BLOCK_NUMBER}" ]]; then
    echo "Set SAYA_FORK_BLOCK_NUMBER to the latest block including the preparations (check here https://sepolia.starkscan.co/, remember to switch to sepolia!)."
    echo "You can now run \`cargo run -r --bin katana -- --rpc-url $SAYA_SEPOLIA_ENDPOINT --fork-block-number \$SAYA_FORK_BLOCK_NUMBER\` in another terminal."
    exit 0
fi

if [[ -z "${SAYA_PILTOVER_ADDRESS}" ]]; then
    sncast -a $SAYA_SNCAST_ACCOUNT_NAME deploy \
        --class-hash $SAYA_PILTOVER_CLASS_HASH \
        -c $SAYA_SEPOLIA_ACCOUNT_ADDRESS $SAYA_PILTOVER_STARTING_STATE_ROOT $(expr $SAYA_FORK_BLOCK_NUMBER + 1) 0 \
        --fee-token eth \
        -u $SAYA_SEPOLIA_ENDPOINT


    echo "Set SAYA_PILTOVER_ADDRESS to the address of the deployed contract."
    exit 0
fi

if [[ -z "${SAYA_PILTOVER_PREPARED}" ]]; then
    sncast -a $SAYA_SNCAST_ACCOUNT_NAME --wait invoke \
        --contract-address $SAYA_PILTOVER_ADDRESS --function set_program_info -c $SAYA_PROGRAM_HASH $SAYA_CONFIG_HASH \
        --fee-token eth \
        -u $SAYA_SEPOLIA_ENDPOINT

    sncast -a $SAYA_SNCAST_ACCOUNT_NAME --wait invoke \
        --contract-address $SAYA_PILTOVER_ADDRESS --function set_facts_registry -c $SAYA_FACT_REGISTRY \
        --fee-token eth \
        -u $SAYA_SEPOLIA_ENDPOINT

fi


if [[ -z "${SAYA_SKIP_MAKING_TRANSACTIONS}" ]]; then
    cargo run -r --bin sozo -- execute dojo_examples-actions spawn \
        --manifest-path $SAYA_MANIFEST_PATH \
        --rpc-url http://localhost:5050 \
        --private-key $SAYA_SEPOLIA_PRIVATE_KEY \
        --account-address $SAYA_SEPOLIA_ACCOUNT_ADDRESS \
        --world $SAYA_WORLD_ADDRESS \
        --wait && \
    cargo run -r --bin sozo -- execute dojo_examples-actions move \
        -c 2 \
        --manifest-path $SAYA_MANIFEST_PATH \
        --rpc-url http://localhost:5050 \
        --private-key $SAYA_SEPOLIA_PRIVATE_KEY \
        --account-address $SAYA_SEPOLIA_ACCOUNT_ADDRESS \
        --world $SAYA_WORLD_ADDRESS \
        --wait
fi


cargo run -r --bin sozo -- model get Moves $SAYA_SEPOLIA_ACCOUNT_ADDRESS \
    --manifest-path $SAYA_MANIFEST_PATH \
    --rpc-url $SAYA_SEPOLIA_ENDPOINT \
    --world $SAYA_WORLD_ADDRESS

cargo run -r --bin saya -- \
    --mode persistent \
    --rpc-url http://localhost:5050 \
    --registry $SAYA_FACT_REGISTRY \
    --settlement-contract $SAYA_PILTOVER_ADDRESS \
    --world $SAYA_WORLD_ADDRESS \
    --prover-url $SAYA_PROVER_URL \
    --store-proofs \
    --starknet-url $SAYA_SEPOLIA_ENDPOINT \
    --signer-key $SAYA_SEPOLIA_PRIVATE_KEY \
    --signer-address $SAYA_SEPOLIA_ACCOUNT_ADDRESS \
    --private-key $SAYA_PROVER_KEY \
    --batch-size 1 \
    --start-block $(expr $SAYA_FORK_BLOCK_NUMBER + 1) \
    --da-chain celestia \
    --celestia-node-url http://localhost:26658 \
    --celestia-namespace saya-dev \
    --celestia-node-auth-token eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJBbGxvdyI6WyJwdWJsaWMiLCJyZWFkIiwid3JpdGUiLCJhZG1pbiJdfQ.kjW6UL2m2XIylCDzG5vwgyW5YA75LyLzXMbnp8Fpe_E
    # --end-block $(expr $SAYA_FORK_BLOCK_NUMBER + 4)
