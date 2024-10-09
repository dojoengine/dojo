#Set all the environment variables in the script for the account setup.

SN_CAST_ACCOUNT_NAME=
DOJO_ACCOUNT_ADDRESS=
SN_CAST_ACCOUNT_TYPE=   #"<braavos|oz|argent>"
DOJO_PRIVATE_KEY=
STARKNET_RPC_URL=
SN_CAST_ACCOUNT_NAME=

sncast account add --name $SN_CAST_ACCOUNT_NAME \
   --address $DOJO_ACCOUNT_ADDRESS \
   --type $SN_CAST_ACCOUNT_TYPE \
   --private-key $DOJO_PRIVATE_KEY \
   --url $STARKNET_RPC_URL \
   --add-profile $SN_CAST_ACCOUNT_NAME