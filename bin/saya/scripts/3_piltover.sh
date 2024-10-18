# Define the account and contract details required for deployment
PILTOVER_CLASS_HASH="0x2a7a2276cf2f00206960ea8a0ea86b1549d6514ab11f546cc71b8154b597c1d"
SAYA_CONFIG_HASH=42
SAYA_PROGRAM_HASH=0x2aa9e430c145b26d681a8087819ed5bff93f5596105d0e74f00fc7caa46fa18
SAYA_FACT_REGISTRY_ADDRESS=0x2cc03dd3136b634bfea2e36e9aac5f966db9576dde3fe43e3ef72e9ece1f42b

# Set the required environment variables
SN_CAST_ACCOUNT_NAME= # The name of the account used with sncast
STARKNET_RPC_URL=     # The Starknet RPC URL to interact with the network
DOJO_ACCOUNT_ADDRESS= # The Dojo account address used for deployment

# Deploy the contract using sncast, and capture the output for transaction and contract address
output=$(sncast -a $SN_CAST_ACCOUNT_NAME deploy \
    -u $STARKNET_RPC_URL \
    --class-hash $PILTOVER_CLASS_HASH \
    --fee-token eth \
    -c $DOJO_ACCOUNT_ADDRESS 0 $((SAYA_FORK_BLOCK_NUMBER + 1)) 0)

# Parse the output to extract the transaction hash and the contract address
TRANSACTION_HASH=$(echo "$output" | grep "transaction_hash:" | awk '{print $2}')
PILTOVER_CONTRACT_ADDRESS=$(echo "$output" | grep "contract_address:" | awk '{print $2}')

# Display the transaction hash and contract address for reference
echo "Piltover deploy transaction hash: $TRANSACTION_HASH"
echo "Piltover contract address: $PILTOVER_CONTRACT_ADDRESS"

# Function to check the finality status of the transaction on L2
check_finality() {
    local tx_hash="$1"    # Transaction hash to check the status for
    local url="$2"        # Starknet RPC URL

    # Use sncast to retrieve the transaction status
    local result=$(sncast --account dev tx-status "$tx_hash" --url "$url")

    # Extract and display the execution and finality statuses
    local execution_status=$(echo "$result" | grep -oP '(?<=execution_status: ).*')
    local finality_status=$(echo "$result" | grep -oP '(?<=finality_status: ).*')

    echo "Execution Status: $execution_status"
    echo "Finality Status: $finality_status"

    # Check if the transaction has been accepted on L2
    if [ "$finality_status" == "AcceptedOnL2" ]; then
        echo "Transaction $tx_hash has been accepted on L2!"
        return 0
    else
        echo "Transaction $tx_hash has not yet been accepted on L2. Waiting..."
        return 1
    fi
}

# Loop to repeatedly check the finality status until the transaction is accepted on L2
while true; do
    check_finality "$TRANSACTION_HASH" "$STARKNET_RPC_URL"
    if [ $? -eq 0 ]; then
        break
    fi
    # Wait for 5 seconds before checking again
    sleep 5
done
echo ""

# Invoke the contract to set program information after deployment
sncast -a $SN_CAST_ACCOUNT_NAME --wait invoke -u $STARKNET_RPC_URL \
        --fee-token eth --contract-address $PILTOVER_CONTRACT_ADDRESS --function set_program_info -c \
        $SAYA_PROGRAM_HASH $SAYA_CONFIG_HASH

echo ""

# Invoke the contract to set the facts registry address
sncast -a $SN_CAST_ACCOUNT_NAME --wait invoke -u $STARKNET_RPC_URL \
        --fee-token eth --contract-address $PILTOVER_CONTRACT_ADDRESS --function set_facts_registry -c \
        $SAYA_FACT_REGISTRY_ADDRESS

echo ""
# Display the final contract address for saving
echo -e "Save piltover address \e[1;32m$PILTOVER_CONTRACT_ADDRESS\e[0m"
