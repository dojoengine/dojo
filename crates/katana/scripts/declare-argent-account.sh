#! /bin/bash

STD_ARGENT_ACCOUNT_0_3_0_CLASS_HASH="0x01a736d6ed154502257f02b1ccdf4d9d1089f80811cd6acad48e6b6a9d1f2003"
STD_ARGENT_ACCOUNT_0_3_0_COMPILED_FILE="../primitives/contracts/compiled/argent_ArgentAccount_0.3.0.json"

STD_ARGENT_ACCOUNT_0_3_1_CLASS_HASH="0x29927c8af6bccf3f6fda035981e765a7bdbf18a2dc0d630494f8758aa908e2b"
STD_ARGENT_ACCOUNT_0_3_1_COMPILED_FILE="../primitives/contracts/compiled/argent_ArgentAccount_0.3.1.json"

TEMP_ACCOUNT_FILE="temp-account.json"
TEMP_KEYSTORE_FILE="temp-keystore.json"

trap "rm -f $TEMP_ACCOUNT_FILE $TEMP_KEYSTORE_FILE" EXIT

# This is helper script is for declaring the standard Argent account to Katana as it doesn't come as a default.
# Argent account contract source code: https://github.com/argentlabs/argent-contracts-starknet/blob/53d01c0d6dce4fd30db955c3d698f658cdda1796/contracts/account/src/argent_account.cairo

# This script expect that you have `starkli` installed.

ACCOUNT_ADDRESS="$1"
PRIVATE_KEY="$2"
RPC_URL="$3"
ARGENT_CLASS_HASH="$4"

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

CLASS_TO_DECLARE=()

if [[ "$ARGENT_CLASS_HASH" == "$STD_ARGENT_ACCOUNT_0_3_0_CLASS_HASH" ]]; then
	CLASS_TO_DECLARE+=($STD_ARGENT_ACCOUNT_0_3_0_COMPILED_FILE)
elif [[ "$ARGENT_CLASS_HASH" == "$STD_ARGENT_ACCOUNT_0_3_1_CLASS_HASH" ]]; then
	CLASS_TO_DECLARE+=($STD_ARGENT_ACCOUNT_0_3_1_COMPILED_FILE)
elif [[ -z "$ARGENT_CLASS_HASH" ]]; then
	# If no specific class hash is provided, declare all the classes
	CLASS_TO_DECLARE=($STD_ARGENT_ACCOUNT_0_3_0_COMPILED_FILE $STD_ARGENT_ACCOUNT_0_3_1_COMPILED_FILE)
else
	echo "Error: Unrecognized Argent account class hash."
	echo "Only supported class hashes are: $STD_ARGENT_ACCOUNT_0_3_0_CLASS_HASH (0.3.0), $STD_ARGENT_ACCOUNT_0_3_1_CLASS_HASH (0.3.1)"
	exit 1
fi


starkli account fetch $ACCOUNT_ADDRESS --rpc $RPC_URL --output $TEMP_ACCOUNT_FILE &> /dev/null

for file in "${CLASS_TO_DECLARE[@]}"; do
	starkli declare --account $TEMP_ACCOUNT_FILE $file --private-key $PRIVATE_KEY --rpc $RPC_URL
done
