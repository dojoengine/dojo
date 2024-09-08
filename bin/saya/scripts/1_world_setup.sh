# Setup the world to be deployed in the context of the Saya example:
#
# Before running the script, please make sure to have the following environment variables set:
#
#  export STARKNET_RPC_URL="https://api.cartridge.gg/x/starknet/sepolia"
#  export DOJO_ACCOUNT_ADDRESS="<YOUR_ACCOUNT_ADDRESS>"
#  export DOJO_PRIVATE_KEY="<YOUR_PRIVATE_KEY>"
#
#  OR use a keystore (if you don't use the DOJO_KEYSTORE_PASSWORD, the password will be asked during the script):
#
#  export DOJO_KEYSTORE_PATH="<PATH_TO_KEYSTORE>"
#  export DOJO_KEYSTORE_PASSWORD="<KEYSTORE_PASSWORD>"
#
set -e

# Check if starkli is installed.
if ! command -v starkli &> /dev/null
then
    echo "starkli could not be found. Please install it running the following commands:"
    echo "curl https://get.starkli.sh | sh"
    echo "starkliup"
    exit 1
fi

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

# Ensures no previous run corrupted the environment.
unset DOJO_WORLD_ADDRESS

# Migrate the world on chain (Sepolia fees can be high, so we multiply the estimate by 20 here to pass everytime).
# We use -vvv to get more information about the transactions and extract data from it.
cargo run -r --bin sozo -- \
    -P saya \
    migrate apply \
    --manifest-path examples/spawn-and-move/Scarb.toml \
    --fee-estimate-multiplier 20 \
    -vvv

#
# The world is now deployed, you should extract the address and the block number at which it was deployed to set the
# following environment variables:
#
# export DOJO_WORLD_ADDRESS="<WORLD_ADDRESS>"
# export SAYA_FORK_BLOCK_NUMBER="<WORLD_DEPLOYMENT_BLOCK_NUMBER>"
#
# /!\ Be aware that the block number needs to be the block in which the transaction that deploys the world was mined.

# Function to extract a specific key from the [world] section in the deployment manifest.
extract_key() {
    local key=$1
    awk -v key="$key" '
    /^\[world\]/ {in_world=1; next}
    /^\[/ {in_world=0}
    in_world && $1 == key {
        gsub(/^[^=]+=[ ]*"?|"?$/, "")
        print $0
        exit
    }
    ' "$TOML_FILE"
}

TOML_FILE="examples/spawn-and-move/manifests/saya/deployment/manifest.toml"
WORLD_ADDRESS=$(extract_key "address")
TRANSACTION_HASH=$(extract_key "transaction_hash")

# Check if TRANSACTION_HASH and WORLD_ADDRESS are empty
if [ -z "$TRANSACTION_HASH" ] || [ -z "$WORLD_ADDRESS" ]; then
    echo "Error: Could not extract transaction hash or world address from the manifest file."
    echo "Please check the contents of $TOML_FILE and ensure the migration was successful."
    exit 1
fi

while true; do
    status=$(starkli status $TRANSACTION_HASH --network sepolia | jq -r '.finality_status')
    if [ "$status" = "ACCEPTED_ON_L2" ]; then
        receipt=$(starkli receipt $TRANSACTION_HASH --network sepolia)
        block_number=$(echo "$receipt" | jq -r '.block_number')
        echo ""
        echo "World deploy transaction accepted, you can export the following variables:"
        echo "export DOJO_WORLD_ADDRESS=$WORLD_ADDRESS"
        echo "export SAYA_FORK_BLOCK_NUMBER=$block_number"
        break
    else
        echo "World deploy transaction not yet accepted. Current status: $status. Retrying in 1 second..."
        sleep 1
    fi
done
