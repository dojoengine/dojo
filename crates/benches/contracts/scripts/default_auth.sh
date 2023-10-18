#!/bin/bash
set -euo pipefail
pushd $(dirname "$0")/..

export WORLD_ADDRESS="0x223b959926c92e10a5de78a76871fa40cefafbdce789137843df7c7b30e3e0";

# enable system -> component authorizations
COMPONENTS=("Position" "Moves" )

for component in ${COMPONENTS[@]}; do
    sozo auth writer $component spawn --world $WORLD_ADDRESS --account-address 0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973 --private-key 0x1800000000300000180000000000030000000000003006001800006600
done

for component in ${COMPONENTS[@]}; do
    sozo auth writer $component move --world $WORLD_ADDRESS --account-address 0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973 --private-key 0x1800000000300000180000000000030000000000003006001800006600
done

echo "Default authorizations have been successfully set."