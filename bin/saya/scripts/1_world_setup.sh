# Setup the world to be deployed in the context of the Saya example:
#
# Before running the script, please make sure to have the following environment variables set:
#
# - DOJO_PRIVATE_KEY: The private key of the account that will deploy the world.
# - DOJO_ACCOUNT_ADDRESS: The address of the account that will deploy the world.
# - STARKNET_RPC_URL: The RPC URL of the StarkNet network you are deploying to.
#

DOJO_PRIVATE_KEY=
DOJO_ACCOUNT_ADDRESS=
STARKNET_RPC_URL=

set -e

# Check if jq is installed.
if ! command -v jq &> /dev/null
then
    echo "jq could not be found. Please install it with any method listed at:"
    echo "https://jqlang.github.io/jq/download/"
    exit 1
fi

# Build the project to have the manifests and artifacts.
cargo run -r --bin sozo -- \
    -P saya \
    build \
    --manifest-path examples/spawn-and-move/Scarb.toml

# # Ensures no previous run corrupted the environment.
unset DOJO_WORLD_ADDRESS

# Migrate the world on chain (Sepolia fees can be high, so we multiply the estimate by 20 here to pass everytime).
# We use -vvv to get more information about the transactions and extract data from it.
cargo run -r --bin sozo -- \
    -P saya \
    migrate apply \
    --manifest-path examples/spawn-and-move/Scarb.toml \
    --rpc-url $STARKNET_RPC_URL\
    --private-key $DOJO_PRIVATE_KEY \
    --account-address $DOJO_ACCOUNT_ADDRESS \
    --fee-estimate-multiplier 20 \
    -vvv

#
# The world is now deployed, you should extract the address and the block number at which it was deployed to set the
# following environment variables:


# /!\ Be aware that the block number needs to be the block in which the transaction that deploys the world was mined.

# Function to extract a specific key from the [world] section in the deployment manifest.
extract_key() {
    local section=$1
    local key=$2

    awk -v section="$section" -v key="$key" '
    # Track the current section
    /^\[.*\]/ {
        in_section = ($0 == "[" section "]")
        next
    }
    # When in the desired section, extract the key
    in_section && $1 == key {
        gsub(/^[^=]+=[ ]*"?|"?$/, "")
        print $0
        exit
    }
    ' "$TOML_FILE"
}

# Example usage
TOML_FILE="examples/spawn-and-move/manifests/saya/deployment/manifest.toml"
WORLD_ADDRESS=$(extract_key "world" "address")
echo "world address: $WORLD_ADDRESS"

TRANSACTION_HASH=$(extract_key "world" "transaction_hash")
echo "transaction hash: $TRANSACTION_HASH"

RPC_URL=$(extract_key "world.metadata" "rpc_url")
echo "RPC URL: $RPC_URL"

# Check if TRANSACTION_HASH and WORLD_ADDRESS are empty
if [ -z "$TRANSACTION_HASH" ] || [ -z "$WORLD_ADDRESS" ]; then
    echo "Error: Could not extract transaction hash or world address from the manifest file."
    echo "Please check the contents of $TOML_FILE and ensure the migration was successful."
    exit 1
fi


check_finality() {
    local tx_hash="$1"
    local url="$2"
    
    # Call sncast to get transaction status
    local result=$(sncast --account dev tx-status "$tx_hash" --url "$url")

    # Extract finality status from the result
    local execution_status=$(echo "$result" | grep -oP '(?<=execution_status: ).*')
    local finality_status=$(echo "$result" | grep -oP '(?<=finality_status: ).*')

    # Output the statuses for debugging purposes
    echo "Execution Status: $execution_status"
    echo "Finality Status: $finality_status"

    # Check if finality status is AcceptedOnL2
    if [ "$finality_status" == "AcceptedOnL2" ]; then
        echo "Transaction $tx_hash has been accepted on L2!"
        return 0
    else
        echo "Transaction $tx_hash has not yet been accepted on L2. Waiting..."
        return 1
    fi
}

# Loop until finality status is "AcceptedOnL2"
while true; do
    check_finality "$TRANSACTION_HASH" "$STARKNET_RPC_URL"
    if [ $? -eq 0 ]; then
        break
    fi
    # Sleep for a few seconds before checking again
    sleep 1
done