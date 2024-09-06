# With sncast, you must first initialize your account. Once the account is initialized,
# and the configuration file is available, you can run the sncast commands referring to your account by name.
#
# Define the account name to be re-used.
SN_CAST_ACCOUNT_NAME="glihm-sep"
SN_CAST_ACCOUNT_TYPE="<braavos|oz|argent>"

# Use existing values if defined, otherwise set default values
: ${SAYA_FACT_REGISTRY_ADDRESS:="<ADDRESS>"}
: ${DOJO_ACCOUNT_ADDRESS:="<ADDRESS>"}
: ${DOJO_PRIVATE_KEY:="<PRIVATE_KEY>"}
: ${SAYA_FORK_BLOCK_NUMBER:=1000000}
: ${STARKNET_RPC_URL:="https://api.cartridge.gg/x/starknet/sepolia"}

PILTOVER_CLASS_HASH="0x01dbe90a725edbf5e03dcb1f116250ba221d3231680a92894d9cc8069f209bd6"
SAYA_CONFIG_HASH=42
SAYA_PROGRAM_HASH=0x1d55211dd0a3d906104984e010fe31d3eb50f20ec6db04b5b4b733db3b3907

# Use this to initialize your account.
#sncast account add --name $SN_CAST_ACCOUNT_NAME \
#    --address $DOJO_ACCOUNT_ADDRESS \
#    --type $SN_CAST_ACCOUNT_TYPE \
#    --private-key $DOJO_PRIVATE_KEY \
#    --url $STARKNET_RPC_URL \
#    --add-profile $SN_CAST_ACCOUNT_NAME

output=$(sncast -a $SN_CAST_ACCOUNT_NAME deploy \
    -u $STARKNET_RPC_URL \
    --class-hash $PILTOVER_CLASS_HASH \
    --fee-token eth \
    -c $DOJO_ACCOUNT_ADDRESS 0 $((SAYA_FORK_BLOCK_NUMBER + 1)) 0)

# sncast doesn't have a --wait flag, so we need to wait for the transaction to be accepted on L2 to continue.
TRANSACTION_HASH=$(echo "$output" | grep "transaction_hash:" | awk '{print $2}')
PILTOVER_CONTRACT_ADDRESS=$(echo "$output" | grep "contract_address:" | awk '{print $2}')

while true; do
    status=$(starkli status $TRANSACTION_HASH --network sepolia | jq -r '.finality_status')
    if [ "$status" = "ACCEPTED_ON_L2" ]; then
        echo ""
        echo "Piltover contract deployed successfully, you can now set the environment variable:"
        echo "export SAYA_PILTOVER_ADDRESS=$PILTOVER_CONTRACT_ADDRESS"
        break
    else
        echo "Piltover deploy transaction not yet accepted. Current status: $status. Retrying in 1 second..."
        sleep 1
    fi
done

echo ""

sncast -a $SN_CAST_ACCOUNT_NAME --wait invoke -u $STARKNET_RPC_URL \
        --fee-token eth --contract-address $PILTOVER_CONTRACT_ADDRESS --function set_program_info -c \
        $SAYA_PROGRAM_HASH $SAYA_CONFIG_HASH

echo ""

sncast -a $SN_CAST_ACCOUNT_NAME --wait invoke -u $STARKNET_RPC_URL \
        --fee-token eth --contract-address $PILTOVER_CONTRACT_ADDRESS --function set_facts_registry -c \
        $SAYA_FACT_REGISTRY_ADDRESS
