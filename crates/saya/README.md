# Saya: settlement service

Saya is a settlement service for Katana.

## Data availability (DA)

Katana being a Starknet sequencer, the [state update](https://docs.starknet.io/documentation/architecture_and_concepts/Network_Architecture/on-chain-data) have to be published on the data availability layer.

Saya is responsible of fetching the state updates from katana to then publish them on the configured DA layer.

## Cairo execution trace

When Katana operates, the internal Starknet state of Katana is updated at each transaction being executed. However, this execution is not run in `proof` mode. This means we have to execute a program named `Starknet OS` where every transaction of a block is replayed.

In the case of the `Starknet OS` run, the `proof` mode is enabled and thus we obtain a cairo execution trace which attest of the correct execution of every transaction of the block.

Saya use [SNOS in rust](https://github.com/keep-starknet-strange/snos) to run the `Starknet OS` with the Cairo VM.

Once the cairo execution trace (PIE format) is generated, it can be sent to a prover.

It's important to note that at this point, we can compute what's called a `fact`, which will be used to generate the proof on.
This `fact` is a hash of the class hash of `Starknet OS` cairo program, combined to the hash of the program's output.
The hash function depends on which verifier will be used (keccak, perdersen, poseidon, ...).

## Prover

The prover is the service responsible of generating a proof for the given `Starknet OS` output.

Saya will be able to use several provers:

- **SHARP**: a StarkWare shared proving service. This service generates the proof AND send the proof and the facts on Ethereum directly.
- **Stone**: [Stone](https://github.com/starkware-libs/stone-prover) is being used to generate the proof associated with the [cairo verifier written by Herodotus](https://github.com/HerodotusDev/cairo-verifier).
- **Platinum**: The [Platinum](https://github.com/lambdaclass/lambdaworks) prover from LambdaClass.

## Verifier and facts registry

The on-chain verifier options so far are:

- **Ethereum**: StarkWare contracts on Ethereum which are tailored to receive the SHARP proofs and facts.
- **Starknet**: Soon, the cairo verifier from Herodotus will enable verification on Starknet.

A verifier comes along a fact registry. A fact registry keep track of which fact (the hash of the program class hash of `Starknet OS` in our case and the hash of it's output) has been proven.

## Library architecture

Currently, Saya contains only module to have the first skeleton of a working service. The idea is to then migrate into crates for each of the components.

The next big step is to have compatibility with SNOS in rust, which is the library responsible of generating the cairo execution trace.

Some work to be done:

1. Add a RPC server to query data from Saya and current progress.
2. Add some parallelism when it's possible, as Saya will inevitably be lagging due to the settlement layer being slower than Katana.

## Dependencies

SNOS, responsible for the cairo execution trace generation, works with Cairo VM main branch with a specific feature.

As one of it's inputs, SNOS in rust requires a `Vec<TransactionExecutionInfo>`, containing the execution info of each transaction of the block. This info is not (yet) stored by Katana neither serializable.

To ensure we've the exact same result, Saya must run the same version (or at least compatible) of the Cairo VM of Katana to replay all the transaction and get their `TransactionExecutionInfo`.

In new Cairo VM version, there are breaking changes as mentioned in the release not, which implies a bump of Cairo VM for Katana and at the same time we could bump to cairo `2.5.0`.
However, papyrus and blockifier which we depend on are still in `-dev` version, where also some breaking changes must be addressed.

- Cairo VM (currently dojo is using 0.8, and others are in 0.9)
- Blockifier (uses Cairo VM and cairo-lang `-dev`)
- Papyrus (used by blockifier and use blockifier and cairo-lang `-dev`)
- cairo-lang (we should support `2.5` now)
- scarb (breaking changes between 2.4 and 2.5 to be addresses, not required to only build saya and SNOS)

## Local Testing

```bash
cargo run -r -p katana # Start an appchain
cargo run -r -p sozo -- build --manifest-path examples/spawn-and-move/Scarb.toml
cargo run -r -p sozo -- migrate apply --rpc-url http://localhost:5050 --manifest-path examples/spawn-and-move/Scarb.toml # Make some transactions
cargo run -r --bin saya -- --rpc-url http://localhost:5050 --registry 0x217746a5f74c2e5b6fa92c97e902d8cd78b1fabf1e8081c4aa0d2fe159bc0eb --world ... # Run Saya
```

## End to end testing

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
    --rpc-url <SEPOIA_ENDPOINT> \
    --private-key <SEPOIA_PRIVATE_KEY> \
    --account-address <SEPOIA_ACCOUNT_ADDRESS> \
    --fee-estimate-multiplier 20 \
    --name <WORLD_NAME>
```

3. Set world configs

```bash
sncast \
    -u <SEPOIA_ENDPOINT> \
    -a dev \
    invoke \
    -a <WORLD_ADDRESS> \
    -f set_differ_program_hash \
    -c 0xa73dd9546f9858577f9fdbe43fd629b6f12dc638652e11b6e29155f4c6328 \
    --max-fee 644996534717092

sleep 3

sncast \
    -u <SEPOIA_ENDPOINT> \
    -a dev \
    invoke \
    -a <WORLD_ADDRESS> \
    -f set_merger_program_hash \
    -c 0xc105cf2c69201005df3dad0050f5289c53d567d96df890f2142ad43a540334 \
    --max-fee 644996534717092

sleep 3

sncast \
    -u <SEPOIA_ENDPOINT> \
    -a dev \
    invoke \
    -a <WORLD_ADDRESS> \
    -f set_facts_registry \
    -c <FACT_REGISTRY> \
    --max-fee 644996534717092
```

4. Start katana

```bash
cargo run -r -p katana -- \
    --rpc-url <SEPOIA_ENDPOINT> \
    --fork-block-number <LATEST_BLOCK> \
    -p 5050
```

5. Run transactions on `katana`

```bash
cargo run -r -p sozo -- execute \
    --rpc-url http://localhost:5050 \
    --private-key <SEPOIA_PRIVATE_KEY> \
    --account-address <SEPOIA_ACCOUNT_ADDRESS> \
    --world <WORLD_ADDRESS> \
    <CONTRACT_ADDRESS> spawn

```

6. Run saya

The <PROVER_URL> is a `http://prover.visoft.dev:3618` or a link to a self hosted instance of `https://github.com/neotheprogramist/http-prover`.
The <PROVER_KEY> is the private key produced by `cargo run -p keygen` in the above repository. Pass the public key to server operator or the prover program.

```bash
cargo run -r --bin saya -- \
    --rpc-url http://localhost:5050 \
    --registry <FACT_REGISTRY> \
    --world <WORLD_ADDRESS> \
    --prover-url <PROVER_URL> \
    --prover-key <PROVER_KEY> \
    --batch-size 2 \
    --start-block <LATEST_BLOCK>
```

## Additional documentation

[Hackmd note](https://hackmd.io/@glihm/saya)
[Overview figma](https://www.figma.com/file/UiQkKjOpACcWihQbF70BbF/Technical-overview?type=whiteboard&node-id=0%3A1&t=0ebbPYytFmDfAkj5-1)
