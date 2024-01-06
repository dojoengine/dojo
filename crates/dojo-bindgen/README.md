# Dojo bindings generator

This crate contains the Dojo bindings generator modules which leverage [cainome](https://github.com/cartridge-gg/cainome) to parse Cairo ABI.

## Architecture

`dojo-bindgen` aims at decoupling at most the knowledge required by `sozo` to output bindings along the contract artifacts. Cainome exposes the `parser` crate, which contains common functions to work with Cairo ABI and generate a list of tokens to have a intermediate representation of the ABI usable at runtime and build logic on top of it to generate the bindings.

[PluginManager](./src/lib.rs): The `PluginManager` is the top level interface that `sozo` uses to request code generation. By providing the artifacts path and the list of plugins (more params in the future), `sozo` indicates which plugin must be invoke to generate the bindings.

[BuiltinPlugin](./src/plugins/mod.rs): The `BuiltinPlugin` are a first lightweight and integrated plugins that are written in rust directly inside this crate. This also comes packaged into the dojo toolchain, ready to be used by developers.

In the future, `dojo-bindgen` will expose a `Plugin` interface similar to protobuf to communicate with a user defined plugin using `stdin` for greater flexibility.

## Builtin Plugins

[Typescript](./src/plugins/typescript/mod.rs)

[Unity](./src/plugins/unity/mod.rs)
