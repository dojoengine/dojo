# Dojo bindings generator

This crate contains the Dojo bindings generator modules which leverage [cainome](https://github.com/cartridge-gg/cainome) to parse Cairo ABI.

## Architecture

Cainome exposes the `parser` crate, which contains common functions to work with Cairo ABI. This is not (yet) a pure plugin system like protobuf can propose. However, `dojo-bindgen` is implemented with plugin fashion in mind, where modularity allow easy evolution of each backend for which code generation can be very different.

`dojo-bindgen` aims at decoupling at most the knowledge required by `sozo` to output bindings along the contract artifacts.

[BindingManager](./src/lib.rs): The `BindingManager` is the top level interface that `sozo` uses to request code generation. By providing the artifacts path and the list of backends (more params in the future), `sozo` indicates which backend must be processed.

[BindingBuilder](./src/backends/mod.rs): The `BindingBuilder` is responsible of generating the code for a specific backend, in a totally independent fashion. The `BindingManager` provides the required inputs like the contract name and the list of tokens for this.

## Backends

[Typescript](./src/backends/typescript/mod.rs)
[Unity](./src/backends/unity/mod.rs)
