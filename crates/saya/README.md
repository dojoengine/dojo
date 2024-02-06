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
* **SHARP**: a StarkWare shared proving service. This service generates the proof AND send the proof and the facts on Ethereum directly.
* **Stone**: [Stone](https://github.com/starkware-libs/stone-prover) is being used to generate the proof associated with the [cairo verifier written by Herodotus](https://github.com/HerodotusDev/cairo-verifier).
* **Platinum**: The [Platinum](https://github.com/lambdaclass/lambdaworks) prover from LambdaClass.

## Verifier and facts registry

The on-chain verifier options so far are:
* **Ethereum**: StarkWare contracts on Ethereum which are tailored to receive the SHARP proofs and facts.
* **Starknet**: Soon, the cairo verifier from Herodotus will enable verification on Starknet.

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

* Cairo VM (currently dojo is using 0.8, and others are in 0.9)
* Blockifier (uses Cairo VM and cairo-lang `-dev`)
* Papyrus (used by blockifier and use blockifier and cairo-lang `-dev`)
* cairo-lang (we should support `2.5` now)
* scarb (breaking changes between 2.4 and 2.5 to be addresses, not required to only build saya and SNOS)

## Additional documentation

[Hackmd note](https://hackmd.io/@glihm/saya)
[Overview figma](https://www.figma.com/file/UiQkKjOpACcWihQbF70BbF/Technical-overview?type=whiteboard&node-id=0%3A1&t=0ebbPYytFmDfAkj5-1)
