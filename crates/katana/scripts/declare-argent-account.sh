#! /bin/bash

ARGENT_ACCOUNT_COMPILED_FILE="../primitives/contracts/compiled/argent-account.json"

TEMP_ACCOUNT_FILE="temp-account.json"
TEMP_KEYSTORE_FILE="temp-keystore.json"

trap "rm -f $TEMP_ACCOUNT_FILE $TEMP_KEYSTORE_FILE" EXIT

# This is helper script is for declaring the standard Argent account to Katana as it doesn't come as a default.
# Argent account contract source code: https://github.com/argentlabs/argent-contracts-starknet/blob/53d01c0d6dce4fd30db955c3d698f658cdda1796/contracts/account/src/argent_account.cairo

# This script expect that you have `starkli` installed.

ACCOUNT_ADDRESS="$1"
PRIVATE_KEY="$2"
RPC_URL="$3"
CLASS_TO_DECLARE="$4"

if [[ -z "$ACCOUNT_ADDRESS" ]] || [[ -z "$PRIVATE_KEY" ]] || [[ -z "$RPC_URL" ]]; then
	# Make sure that the account address and private key are provided
	if [[ -z "$ACCOUNT_ADDRESS" ]]; then
	    echo "Error: Account address is not provided."
	elif [[ -z "$PRIVATE_KEY" ]]; then
	    echo "Error: Private key is not provided."
	elif [[ -z "$RPC_URL" ]]; then
	    echo "Error: Rpc url is not provided."
	fi

	echo "Usage: declare-argent-account.sh <ACCOUNT_ADDRESS> <PRIVATE_KEY> <RPC_URL>"
	exit 1
fi

# Check if `starkli` exists
if ! command -v starkli &> /dev/null
then
    echo "Error: `starkli` not found."
    exit 1
fi

if [[ -z "$CLASS_TO_DECLARE" ]]; then
    CLASS_TO_DECLARE=$ARGENT_ACCOUNT_COMPILED_FILE
fi

starkli account fetch $ACCOUNT_ADDRESS --rpc $RPC_URL --output $TEMP_ACCOUNT_FILE &> /dev/null
starkli declare --account $TEMP_ACCOUNT_FILE $CLASS_TO_DECLARE --private-key $PRIVATE_KEY --rpc $RPC_URL
