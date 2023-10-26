#!/bin/bash
set -euo pipefail
pushd $(dirname "$0")/..

export WORLD_ADDRESS="0x223b959926c92e10a5de78a76871fa40cefafbdce789137843df7c7b30e3e0";
export ACCOUNT_ADDRESS="0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973";
export PRIVATE_KEY="0x1800000000300000180000000000030000000000003006001800006600";


# enable system -> component authorizations
COMPONENTS=("Position" "Moves" "Alias" )

for component in ${COMPONENTS[@]}; do
    sozo auth writer $component spawn --world $WORLD_ADDRESS --account-address $ACCOUNT_ADDRESS --private-key $PRIVATE_KEY
done

for component in ${COMPONENTS[@]}; do
    sozo auth writer $component move --world $WORLD_ADDRESS --account-address $ACCOUNT_ADDRESS --private-key $PRIVATE_KEY
done

sozo auth writer Alias bench_emit --world $WORLD_ADDRESS --account-address $ACCOUNT_ADDRESS --private-key $PRIVATE_KEY
sozo auth writer Alias bench_set --world $WORLD_ADDRESS --account-address $ACCOUNT_ADDRESS --private-key $PRIVATE_KEY
sozo auth writer Alias bench_get --world $WORLD_ADDRESS --account-address $ACCOUNT_ADDRESS --private-key $PRIVATE_KEY


echo "Default authorizations have been successfully set."