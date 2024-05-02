
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

### 1. Deploy a New World Contract
First, deploy a new smart contract using the Saya framework:
```bash
cargo run -r -p sozo -- build --manifest-path examples/spawn-and-move/Scarb.toml

cargo run -r -p sozo -- migrate apply --manifest-path examples/spawn-and-move/Scarb.toml --rpc-url <> --fee-estimate-multiplier 1000 --private-key <> --account-address <> --name saya-world-v1
```

### 2. Initialize a Local Katana Instance
Start a local instance of Katana configured to work with the newly deployed contract:
```bash
cargo run -r -p katana -- -b 30000 --rpc-url <> --fork-block-number <block number after world deployment>
```

### 3. Launch the Saya Service
Execute the Saya process to interact with the blockchain network:
```bash
cargo run -r --bin saya -- --rpc-url http://localhost:5050 --registry 0x217746a5f74c2e5b6fa92c97e902d8cd78b1fabf1e8081c4aa0d2fe159bc0eb --world <> --start-block <>
```

Currently the Stone prover in use is optimized for AMD64. Hence, running it on a ARM64 machine will be relatively slow or even not compatible if emulation is now available.
If you can't run Saya on a AMD64 machine, you may choose to use the HTTP wrapper currently proposed by Visoft.
```bash
cargo run -r --bin saya -- --rpc-url http://localhost:5050 --prover-url http://prover.visoft.dev:3618/prove/state-diff-commitment --registry 0x217746a5f74c2e5b6fa92c97e902d8cd78b1fabf1e8081c4aa0d2fe159bc0eb --world <> --start-block <>
```

### 4. Modify the World State Using `sozo`
Finally, modify the state of the world using specific actions:
```bash
cargo run -r -p sozo -- execute --rpc-url http://localhost:5050 --private-key <> --account-address <> --world <> dojo_examples::actions::actions spawn
```

After this command, Saya will pick up the blocks with transactions, generate the proof for the state transition, and send it to the base layer world contract.

Ensure to replace placeholders (`<>`) with appropriate values for your configuration and environment. This documentation provides a comprehensive overview for developers and operators to effectively utilize the Saya service in blockchain applications.
