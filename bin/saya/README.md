# Saya Executable Documentation

This documentation outlines the operation of the Saya executable, a CLI-based service designed to interact with blockchain components for state management and updates. Saya supports operations on Celestia nodes and integrates with Katana blocks to provide a streamlined blockchain interaction process.

## Key Features

- **Celestia Node Integration**: Allows publishing state updates to a Celestia node (WIP).
- **Katana Block Fetching**: Saya can fetch blocks from Katana, aiding in local blockchain simulations and testing.

## Prerequisites

Ensure you have the following set up:

- Rust and Cargo installed on your system.
- Access to Celestia and/or Katana node URLs if needed.

## Basic Usage Example

Below is a command-line example that demonstrates how to run the Saya executable with necessary parameters:

```bash
cargo run --bin saya -- --rpc-url http://localhost:5050 --da-chain celestia --celestia-node-url http://127.0.0.1:26658 --celestia-namespace mynm --celestia-node-auth-token eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.....
```

## Detailed Workflow

1. Prepare fact registry contract

   Declare or use already declared `class-hash`: `0x7f6076572e04d7182a1c5c9f1f4c15aafcb069b1bfdb3de4d7c9e47c99deeb4`.

   Deploy or use already deployed `contract`: `0x217746a5f74c2e5b6fa92c97e902d8cd78b1fabf1e8081c4aa0d2fe159bc0eb`.

   In the repository https://github.com/HerodotusDev/integrity run

```bash
    fact_registry/1-declare.sh # extract `class-hash`
    fact_registry/1-deploy.sh <CLASS_HASH> # use at <FACT_REGISTRY>
```

2. Spawn world

You must choose a world's name as you may deploy the exact same code as an other person trying this example. The world's name must fit into 31 characters.

```bash
cargo run -r --bin sozo -- \
    build \
    --manifest-path examples/spawn-and-move/Scarb.toml

cargo run -r --bin sozo -- \
    migrate apply \
    --manifest-path examples/spawn-and-move/Scarb.toml \
    --rpc-url <SEPOLIA_ENDPOINT> \
    --private-key <SEPOLIA_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --fee-estimate-multiplier 20 \
    --name <WORLD_NAME>
```

Once the migration is done, please take note of the address of the world as it will be re-used in the commands below.

1. Set world configs

```bash
cargo run -r --bin sozo -- \
    execute <WORLD_ADDRESS> set_differ_program_hash \
    -c 0xa73dd9546f9858577f9fdbe43fd629b6f12dc638652e11b6e29155f4c6328 \
    --manifest-path examples/spawn-and-move/Scarb.toml \
    --rpc-url <SEPOLIA_ENDPOINT> \
    --private-key <SEPOLIA_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --fee-estimate-multiplier 20 \
    --world <WORLD_ADDRESS> \
    --wait

cargo run -r --bin sozo -- \
    execute <WORLD_ADDRESS> set_merger_program_hash \
    -c 0xc105cf2c69201005df3dad0050f5289c53d567d96df890f2142ad43a540334 \
    --manifest-path examples/spawn-and-move/Scarb.toml \
    --rpc-url <SEPOLIA_ENDPOINT> \
    --private-key <SEPOLIA_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --fee-estimate-multiplier 20 \
    --world <WORLD_ADDRESS> \
    --wait

cargo run -r --bin sozo -- \
    execute <WORLD_ADDRESS> set_facts_registry \
    -c 0x217746a5f74c2e5b6fa92c97e902d8cd78b1fabf1e8081c4aa0d2fe159bc0eb \
    --manifest-path examples/spawn-and-move/Scarb.toml \
    --rpc-url <SEPOLIA_ENDPOINT> \
    --private-key <SEPOLIA_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --fee-estimate-multiplier 20 \
    --world <WORLD_ADDRESS> \
    --wait
```
4. Preparing Piltover Contract
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
5. Start katana

Start a local instance of Katana configured to work with the newly deployed contract. You should wait your world to be integrated into the latest block (and not the pending).
Once block in which the transaction that deploys the world is mined, you can start `katana` in forking mode.

```bash
cargo run -r --bin katana -- \
    --rpc-url <SEPOLIA_ENDPOINT> \
    --fork-block-number <LATEST_BLOCK>
```

5. Run transactions on `katana`

Finally, modify the state of the world using specific actions:

```bash
cargo run -r --bin sozo -- execute dojo_examples::actions::actions spawn \
    --manifest-path examples/spawn-and-move/Scarb.toml \
    --rpc-url http://localhost:5050 \
    --private-key <SEPOLIA_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --world <WORLD_ADDRESS> \
    --wait
```

Before running `saya`, we can check the actual value for some models on Sepolia, to then see them updated by the proof being verified and the state of the world being updated.
In the `spawn-and-move` example, the `Position` model is used to store some data associated with the player,
being the contract address of the contract that called `spawn` (hence, your account address).
By default on Sepolia, it should be set like to unknown position, being like:

```bash
cargo run -r --bin sozo -- model get Position <ACCOUNT_ADDRESS> \
    --manifest-path examples/spawn-and-move/Scarb.toml \
    --rpc-url <SEPOLIA_ENDPOINT> \
    --world <WORLD_ADDRESS>
```

```json
// Expected on Sepolia as we've executed the transaction on the Katana shard.
{
    player          : <SEPOLIA_ACCOUNT_ADDRESS>,
    vec             : {
        x               : 0,
        y               : 0
    }
}

// Expected on Katana.
{
    player          : <SEPOLIA_ACCOUNT_ADDRESS>,
    vec             : {
        x               : 10,
        y               : 10
    }
}
```

6. Run saya

The <PROVER_URL> could be `http://prover.visoft.dev:3618` if you have a registered key or a link to a self hosted instance of `https://github.com/neotheprogramist/http-prover`.
The <PROVER_KEY> is the private key produced by `keygen` installed with `cargo install --git https://github.com/neotheprogramist/http-prover keygen`. Pass the public key to server operator or the prover program.

If you are on an `amd64` architecture, go ahead and run the `http-prover` locally to see how it works and run this whole pipeline locally.
If not (this includes Apple Silicon), some emulation will take place to run the prover on your machine, and this is very very slow.

It's important that the `--start-block` of Saya is the first block produced by Katana as for now Katana is not fetching events from the forked network.

Starknet sepolia network chain id is `0x00000000000000000000000000000000000000000000534e5f5345504f4c4941`.

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

After this command, Saya will pick up the blocks with transactions, generate the proof for the state transition, and send it to the base layer world contract.

Once the world on Sepolia is updated, you can issue again the `model get` command as seen before, and you should see the `katana` shard state reflected on Sepolia.

Ensure to replace placeholders (`<>`) with appropriate values for your configuration and environment. This documentation provides a comprehensive overview for developers and operators to effectively utilize the Saya service in blockchain applications.