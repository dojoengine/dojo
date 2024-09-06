# Configure the world by setting the differ, merger and facts registry programs.

cargo run -r --bin sozo -- \
    execute $DOJO_WORLD_ADDRESS set_differ_program_hash \
    -c 0xa73dd9546f9858577f9fdbe43fd629b6f12dc638652e11b6e29155f4c6328 \
    --manifest-path examples/spawn-and-move/Scarb.toml \
    --fee-estimate-multiplier 20 \
    --wait

cargo run -r --bin sozo -- \
    execute $DOJO_WORLD_ADDRESS set_merger_program_hash \
    -c 0xc105cf2c69201005df3dad0050f5289c53d567d96df890f2142ad43a540334 \
    --manifest-path examples/spawn-and-move/Scarb.toml \
    --fee-estimate-multiplier 20 \
    --wait

cargo run -r --bin sozo -- \
    execute $DOJO_WORLD_ADDRESS set_facts_registry \
    -c 0x217746a5f74c2e5b6fa92c97e902d8cd78b1fabf1e8081c4aa0d2fe159bc0eb \
    --manifest-path examples/spawn-and-move/Scarb.toml \
    --fee-estimate-multiplier 20 \
    --wait
