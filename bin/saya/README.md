# Saya Executable Documentation

This documentation outlines the operation of the Saya executable, a CLI-based service designed to interact with blockchain components for state management and updates. Saya supports operations on Celestia nodes and integrates with Katana blocks to provide a streamlined blockchain interaction process.


## Prerequisites

Ensure you have the following set up:

- Rust and Cargo installed on your system.
- Access to Katana node URLs if needed.

## Detailed Workflow For Persistent Mode with Shard Dungeon example
1. Clone repo with Shard Dungeon next to Dojo repository   `git clone https://github.com/neotheprogramist/shard-dungeon.git`  

2. Spawn world

You must choose a world's name as you may deploy the exact same code as an other person trying this example. The world's name must fit into 31 characters.

```bash
sozo build --manifest-path ../shard-dungeon/Scarb.toml

sozo migrate apply \
    --manifest-path ../shard-dungeon/Scarb.toml \
    --rpc-url <SEPOLIA_ENDPOINT> \
    --private-key <SEPOLIA_ACCOUNT_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --fee-estimate-multiplier 20 \
    --name <WORLD_NAME>
```
Once the migration is done, please take note of the address of the world as it will be re-used in the commands below.

3. Preparing world

```bash 
sozo \
    execute <SAYA_WORLD_ADDRESS> set_differ_program_hash \
    -c 2265722951651489608338464389196546125983429710081933755514038580032192121109 \
    --manifest-path ../shard-dungeon/Scarb.toml \
    --rpc-url <SEPOLIA_ENDPOINT> \
    --private-key <SEPOLIA_ACCOUNT_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --fee-estimate-multiplier 20 \
    --world <WORLD_NAME> \
    --wait

sozo \
    execute <SAYA_WORLD_ADDRESS> set_merger_program_hash \
    -c 2265722951651489608338464389196546125983429710081933755514038580032192121109 \
    --manifest-path ../shard-dungeon/Scarb.toml \
    --rpc-url <SEPOLIA_ENDPOINT> \
    --private-key <SEPOLIA_ACCOUNT_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --fee-estimate-multiplier 20 \
    --world <WORLD_NAME> \
    --wait

sozo \
    execute <SAYA_WORLD_ADDRESS> set_facts_registry \
    -c <SAYA_FACT_REGISTRY_ADDRESS> \
    --manifest-path ../shard-dungeon/Scarb.toml \
    --rpc-url <SEPOLIA_ENDPOINT> \
    --private-key <SEPOLIA_ACCOUNT_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --fee-estimate-multiplier 20 \
    --world <WORLD_NAME> \
    --wait    
```

4.
Set SAYA_FORK_BLOCK_NUMBER to the latest block including the preparations (check here https://sepolia.starkscan.co/, remember to switch to sepolia!)."  

You can now run `cargo run -r --bin katana -- --rpc-url <SEPOLIA_ENDPOINT> --fork-block-number <SAYA_FORK_BLOCK_NUMBER>` in another terminal."

5. Preparing Piltover Contract
```bash 
sncast -a <SAYA_SNCAST_ACCOUNT_NAME> -u <SEPOLIA_ENDPOINT> deploy \
        --class-hash <SAYA_PILTOVER_CLASS_HASH> \
        -c <SEPOLIA_ACCOUNT_ADDRESS> 0 <SAYA_FORK_BLOCK_NUMBER + 1>  0
```
Save the address of the deployed contract, we will use it later as SAYA_PILTOVER_ADDRESS
```bash
sncast -a <SAYA_SNCAST_ACCOUNT_NAME> -u <SEPOLIA_ENDPOINT> --wait invoke \
    --contract-address <SAYA_PILTOVER_ADDRESS> --function set_program_info -c 0x042066b8031c907125abd1acb9265ad2ad4b141858d1e1e3caafb411d9ab71cc 42
```
```bash
sncast -a <SAYA_SNCAST_ACCOUNT_NAME> -u <SEPOLIA_ENDPOINT> --wait invoke \
    --contract-address <SAYA_PILTOVER_ADDRESS> --function set_facts_registry -c <SAYA_FACT_REGISTRY_ADDRESS>
```
6. Making example transactions with ShardDungeon
```bash
sozo execute shard_dungeon::systems::metagame::metagame register_player \
    -c str:mateo \
    --manifest-path ../shard-dungeon/Scarb.toml \
    --rpc-url http://localhost:5050 \
    --private-key <SEPOLIA_ACCOUNT_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --world <SAYA_WORLD_ADDRESS> \
    --wait
```
```bash
sozo execute shard_dungeon::systems::hazard_hall::hazard_hall enter_dungeon \
    --manifest-path ../shard-dungeon/Scarb.toml \
    --rpc-url http://localhost:5050 \
    --private-key <SEPOLIA_ACCOUNT_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --world <SAYA_WORLD_ADDRESS>  \
    --wait
```
```bash
sozo execute shard_dungeon::systems::hazard_hall::hazard_hall fate_strike \
    --manifest-path ../shard-dungeon/Scarb.toml \
    --rpc-url http://localhost:5050 \
    --private-key <SEPOLIA_ACCOUNT_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --world <SAYA_WORLD_ADDRESS>  \
    --wait
```
```bash
sozo execute shard_dungeon::systems::hazard_hall::hazard_hall fate_strike \
    --manifest-path ../shard-dungeon/Scarb.toml \
    --rpc-url http://localhost:5050 \
    --private-key <SEPOLIA_ACCOUNT_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --world <SAYA_WORLD_ADDRESS> \
    --wait
```

```bash
sozo -- model get Inventory <SEPOLIA_ACCOUNT_ADDRESS> \
    --manifest-path ../shard-dungeon/Scarb.toml \
    --rpc-url <SEPOLIA_ENDPOINT> \
    --world <SAYA_WORLD_ADDRESS>
```
7. Finally we can run saya 

```bash
cargo run -r --bin saya -- \
    --mode persistent \
    --rpc-url http://localhost:5050 \
    --registry <SAYA_FACT_REGISTRY_ADDRESS> \
    --piltover <SAYA_PILTOVER_ADDRESS> \
    --world <SAYA_WORLD_ADDRESS> \
    --prover-url <SAYA_PROVER_URL> \
    --store-proofs \
    --starknet-url <SEPOLIA_ENDPOINT> \
    --signer-key <SEPOLIA_ACCOUNT_PRIVATE_KEY> \
    --signer-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --private-key <SAYA_PROVER_KEY> \
    --start-block <SAYA_FORK_BLOCK_NUMBER + 1>
```