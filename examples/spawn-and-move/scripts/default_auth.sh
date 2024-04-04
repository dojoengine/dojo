#!/bin/bash
set -euo pipefail
pushd $(dirname "$0")/..

export RPC_URL="http://localhost:5050";

export WORLD_ADDRESS=$(cat ./manifests/deployments/KATANA.json | jq -r '.world.address')

export ACTIONS_ADDRESS=$(cat ./manifests/deployments/KATANA.json | jq -r '.contracts[] | select(.kind == "DojoContract" ).address')

echo "---------------------------------------------------------------------------"
echo world : $WORLD_ADDRESS 
echo " "
echo actions : $ACTIONS_ADDRESS
echo "---------------------------------------------------------------------------"

# enable system -> component authorizations
COMPONENTS=("Position" "Moves" )

for component in ${COMPONENTS[@]}; do
    sozo auth grant writer $component,$WORLD_ADDRESS
    # time out for 1 second to avoid rate limiting
    sleep 1
done

echo "Default authorizations have been successfully set."
