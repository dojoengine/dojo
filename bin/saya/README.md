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

```bash
cargo run -r -p sozo -- \
    build \
    --manifest-path examples/spawn-and-move/Scarb.toml

cargo run -r -p sozo -- \
    migrate apply \
    --manifest-path examples/spawn-and-move/Scarb.toml \
    --rpc-url <SEPOLIA_ENDPOINT> \
    --private-key <SEPOLIA_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --fee-estimate-multiplier 20 \
    --name <WORLD_NAME>
```

3. Set world configs

```bash
cargo run -r -p sozo -- \
    execute <WORLD_ADDRESS> set_differ_program_hash \
    -c 0xa73dd9546f9858577f9fdbe43fd629b6f12dc638652e11b6e29155f4c6328 \
    --manifest-path examples/spawn-and-move/Scarb.toml \
    --rpc-url <SEPOLIA_ENDPOINT> \
    --private-key <SEPOLIA_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --fee-estimate-multiplier 20
    --wait

cargo run -r -p sozo -- \
    execute <WORLD_ADDRESS> set_merger_program_hash \
    -c 0xc105cf2c69201005df3dad0050f5289c53d567d96df890f2142ad43a540334 \
    --manifest-path examples/spawn-and-move/Scarb.toml \
    --rpc-url <SEPOLIA_ENDPOINT> \
    --private-key <SEPOLIA_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --fee-estimate-multiplier 20
    --wait

cargo run -r -p sozo -- \
    execute <WORLD_ADDRESS> set_facts_registry \
    -c 0x217746a5f74c2e5b6fa92c97e902d8cd78b1fabf1e8081c4aa0d2fe159bc0eb \
    --manifest-path examples/spawn-and-move/Scarb.toml \
    --rpc-url <SEPOLIA_ENDPOINT> \
    --private-key <SEPOLIA_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --fee-estimate-multiplier 20
    --wait
```

4. Start katana

Start a local instance of Katana configured to work with the newly deployed contract:

```bash
cargo run -r -p katana -- \
    --rpc-url <SEPOLIA_ENDPOINT> \
    --fork-block-number <LATEST_BLOCK>
```

5. Run transactions on `katana`

Finally, modify the state of the world using specific actions:

```bash
cargo run -r -p sozo -- execute dojo_examples::actions::actions spawn \
    --manifest-path examples/spawn-and-move/Scarb.toml \
    --rpc-url http://localhost:5050 \
    --private-key <SEPOLIA_PRIVATE_KEY> \
    --account-address <SEPOLIA_ACCOUNT_ADDRESS> \
    --world <WORLD_ADDRESS> \
    --wait
```

6. Run saya

The <PROVER_URL> is a `http://prover.visoft.dev:3618` or a link to a self hosted instance of `https://github.com/neotheprogramist/http-prover`.
The <PROVER_KEY> is the private key produced by `keygen` installed with `cargo install --git https://github.com/neotheprogramist/http-prover keygen`. Pass the public key to server operator or the prover program.

It's important that the `--start-block` of Saya is the first block produced by Katana as for now Katana is not fetching events in forked mode.

```bash
cargo run -r --bin saya -- \
    --rpc-url http://localhost:5050 \
    --registry <FACT_REGISTRY> \
    --world <WORLD_ADDRESS> \
    --url <PROVER_URL> \
    --private-key <PROVER_KEY> \
    --start-block <LATEST_BLOCK_PLUS_1>
```

After this command, Saya will pick up the blocks with transactions, generate the proof for the state transition, and send it to the base layer world contract.

Ensure to replace placeholders (`<>`) with appropriate values for your configuration and environment. This documentation provides a comprehensive overview for developers and operators to effectively utilize the Saya service in blockchain applications.
