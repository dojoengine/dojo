# Set environment variables for the world configuration
DOJO_WORLD_ADDRESS=         # Address of the Dojo World contract
STARKNET_RPC_URL=           # Starknet RPC URL to interact with the network
DOJO_PRIVATE_KEY=           # Private key for executing transactions
DOJO_ACCOUNT_ADDRESS=       # Account address used for signing transactions

# Set the differ program hash in the Dojo World contract
cargo run -r --bin sozo -- \
    execute $DOJO_WORLD_ADDRESS set_differ_program_hash \
    -c 0xa73dd9546f9858577f9fdbe43fd629b6f12dc638652e11b6e29155f4c6328 \
    --manifest-path examples/spawn-and-move/Scarb.toml \  # Path to Scarb project manifest
    --rpc-url $STARKNET_RPC_URL \                         # Starknet RPC URL
    --private-key $DOJO_PRIVATE_KEY \                     # Private key to sign the transaction
    --account-address $DOJO_ACCOUNT_ADDRESS \             # Account address for transaction authorization
    --fee-estimate-multiplier 20 \                        # Set fee estimate multiplier
    --world $DOJO_WORLD_ADDRESS \                         # Specify the world address for the command
    --wait                                                # Wait for the transaction to complete

# Set the merger program hash in the Dojo World contract
cargo run -r --bin sozo -- \
    execute $DOJO_WORLD_ADDRESS set_merger_program_hash \
    -c 0xc105cf2c69201005df3dad0050f5289c53d567d96df890f2142ad43a540334 \
    --manifest-path examples/spawn-and-move/Scarb.toml \  # Path to Scarb project manifest
    --rpc-url $STARKNET_RPC_URL \                         # Starknet RPC URL
    --private-key $DOJO_PRIVATE_KEY \                     # Private key to sign the transaction
    --account-address $DOJO_ACCOUNT_ADDRESS \             # Account address for transaction authorization
    --fee-estimate-multiplier 20 \                        # Set fee estimate multiplier
    --wait                                                # Wait for the transaction to complete

# Set the facts registry in the Dojo World contract
cargo run -r --bin sozo -- \
    execute $DOJO_WORLD_ADDRESS set_facts_registry \
    -c 0x2cc03dd3136b634bfea2e36e9aac5f966db9576dde3fe43e3ef72e9ece1f42b \
    --manifest-path examples/spawn-and-move/Scarb.toml \  # Path to Scarb project manifest
    --rpc-url $STARKNET_RPC_URL \                         # Starknet RPC URL
    --private-key $DOJO_PRIVATE_KEY \                     # Private key to sign the transaction
    --account-address $DOJO_ACCOUNT_ADDRESS \             # Account address for transaction authorization
    --fee-estimate-multiplier 20 \                        # Set fee estimate multiplier
    --world $DOJO_WORLD_ADDRESS \                         # Specify the world address for the command
    --wait                                                # Wait for the transaction to complete
