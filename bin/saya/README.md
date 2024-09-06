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
cargo run --bin saya -- \
--rpc-url http://localhost:5050 \
--da-chain celestia \
--celestia-node-url http://127.0.0.1:26658 \
--celestia-namespace mynm \
--celestia-node-auth-token eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.....
```

## Detailed Workflow

1. Setup your environment:
    * For now starknet foundry is required until Sozo supports deploying non-dojo contracts. Please refer to [install instructions](https://foundry-rs.github.io/starknet-foundry/getting-started/installation.html#installation-via-asdf).
    * Setup some environment variables for `sozo`:
    ```bash
    # Use private key or keystore, only one of them is required. For the keystore, please refer to Starkli documentation: https://book.starkli.rs/signers#encrypted-keystores.
    export STARKNET_RPC_URL="https://api.cartridge.gg/x/starknet/sepolia"
    export DOJO_ACCOUNT_ADDRESS="<YOUR_ACCOUNT_ADDRESS>"
    export DOJO_PRIVATE_KEY="<YOUR_PRIVATE_KEY>"
    ```
    * `sncast` doesn't support environment variables, for now, so you may have to set the options manually.

    During this tutorial, we will export environment variables, so you must remain in the same shell session.

2. Prepare fact registry contract
   Declare or use already declared `class-hash`: `0x7f6076572e04d7182a1c5c9f1f4c15aafcb069b1bfdb3de4d7c9e47c99deeb4`.
   Deploy or use already deployed `contract`: `0x217746a5f74c2e5b6fa92c97e902d8cd78b1fabf1e8081c4aa0d2fe159bc0eb`.

   ```bash
   export SAYA_FACT_REGISTRY_ADDRESS="0x217746a5f74c2e5b6fa92c97e902d8cd78b1fabf1e8081c4aa0d2fe159bc0eb"
   ```

   In the repository https://github.com/HerodotusDev/integrity run

    ```bash
    fact_registry/1-declare.sh # extract `class-hash`
    fact_registry/1-deploy.sh <CLASS_HASH> # use at <FACT_REGISTRY>
    ```

3. Spawn world

    You must choose a different world seed as an other person trying this example will have the same world's address. To modify the world's seed, modify the `seed` parameter in the `examples/spawn-and-move/dojo_saya.toml` file.

    Then execute this command, being at the root of the repository:

    ```bash
    bash bin/saya/scripts/1_world_setup.sh
    ```

    Once the migration is done, the world address and the block number at which the world was deployed will be printed,
    you can setup your environment variable like so:
    ```bash
    export DOJO_WORLD_ADDRESS="<WORLD_ADDRESS>"
    export SAYA_FORK_BLOCK_NUMBER="<WORLD_DEPLOYMENT_BLOCK_NUMBER>"
    ```

    Once those variables are exported, you can run the following command to configure the world:
    ```bash
    bash bin/saya/scripts/2_world_config.sh
    ```

4. Preparing Piltover Contract
    The current Piltover contract is under [Cartridge github](https://github.com/cartridge-gg/piltover) and the class hash is `0x01dbe90a725edbf5e03dcb1f116250ba221d3231680a92894d9cc8069f209bd6`.

    At the moment, we don't have a piltover maintained by Dojo community to receive all state updates for multiple
    appchain, this is coming soon.

    In the meantime, if you need to test the piltover contract, you can deploy your own piltover contract using the following command:
    ```bash
    bash bin/saya/scripts/3_piltover.sh
    ```

5. Start katana

    Start a local instance of Katana configured to work with the newly deployed contract. You should wait your world to be integrated into the **latest block** (and not the pending one).
    Once the block in which the transaction that deploys the world is mined, you can start `katana` in forking mode.

    If you need to start an other terminal, you can first print the variables you need to set:
    ```bash
    echo $STARKNET_RPC_URL
    echo $SAYA_FORK_BLOCK_NUMBER
    ```
    Then start katana with the following command:
    ```bash
    cargo run -r --bin katana -- \
    --rpc-url $STARKNET_RPC_URL \
    --fork-block-number $SAYA_FORK_BLOCK_NUMBER
    ```

6. Run transactions on `katana`

    Finally, modify the state of the world using specific actions and granting some permissions:

    ```bash
    cargo run -r --bin sozo -- auth grant writer ns:dojo_examples,actions \
        --manifest-path examples/spawn-and-move/Scarb.toml \
        --rpc-url http://localhost:5050 \
        --wait

    cargo run -r --bin sozo -- execute actions spawn \
        --manifest-path examples/spawn-and-move/Scarb.toml \
        --rpc-url http://localhost:5050 \
        --wait
    ```

    Before running `saya`, we can check the actual value for some models on Sepolia, to then see them updated by the proof being verified and the state of the world being updated.
    In the `spawn-and-move` example, the `Position` model is used to store some data associated with the player,
    being the contract address of the contract that called `spawn` (hence, your account address).
    By default on Sepolia, it should be set like to unknown position, being like:

    ```bash
    cargo run -r --bin sozo -- model get Position <ACCOUNT_ADDRESS> \
        --manifest-path examples/spawn-and-move/Scarb.toml \
        --rpc-url http://localhost:5050
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

8. Run saya

    The <PROVER_URL> could be `http://localhost:3618` if you have a registered key or a link to a self hosted instance of `https://github.com/cartridge-gg/http-prover`.
    The <PROVER_KEY> is the private key produced by `keygen` installed with `cargo install --git https://github.com/cartridge-gg/http-prover keygen`. Pass the public key to server operator or the prover program.

    You can also use the service provided by cartridge by asking to pre-register your key to the service on the Cartridge discord to experiment with Saya.

    If you are on an `amd64` architecture, go ahead and run the `http-prover` locally to see how it works and run this whole pipeline locally.
    If not (this includes Apple Silicon), some emulation will take place to run the prover on your machine, and this is very very slow.

    It's important that the `--start-block` of Saya is the first block produced by Katana as for now Katana is not fetching events from the forked network. To get this value, you can add one to the `SAYA_FORK_BLOCK_NUMBER` value.

    ```bash
    cargo run -r --bin saya -- \
        --mode persistent \
        --rpc-url http://localhost:5050 \
        --registry $SAYA_FACT_REGISTRY_ADDRESS \
        --settlement-contract $SAYA_PILTOVER_ADDRESS \
        --prover-url <PROVER_URL> \
        --store-proofs \
        --private-key <PROVER_PRIVATE_KEY> \
        --start-block $(($SAYA_FORK_BLOCK_NUMBER + 1))
    ```

    After this command, Saya will pick up the blocks with transactions, generate the proof for the state transition, and send it to the base layer world contract.

    Once the world on Sepolia is updated, you can issue again the `model get` command as seen before, and you should see the `katana` shard state reflected on Sepolia.

    Ensure to replace placeholders (`<>`) with appropriate values for your configuration and environment. This documentation provides a comprehensive overview for developers and operators to effectively utilize the Saya service in blockchain applications.
