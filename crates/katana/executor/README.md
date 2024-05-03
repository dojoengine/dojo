## katana-executor

This crate provides a set of abstractions for performing transaction executions in Katana. It includes [implementations](./src/implementation/) for the different execution engines that Katana supports:

1. [blockifier](https://github.com/dojoengine/blockifier) by StarkWare
2. [starknet_in_rust](https://github.com/dojoengine/starknet_in_rust) by LambdaClass.

They are feature-gated under `blockifier` and `sir` features respectively and enabled by **default**.

This crate also includes a [_noop_](./src/implementation/noop.rs) implementation for testing purposes.

### Cairo Native support

The [starknet_in_rust](./src/implementation/sir/) executor can be integrated with Cairo Native, which makes the execution of sierra programs possible through native machine code. To use it, you must enable the `native` feature when using this crate as a dependency,

```toml
[dependencies]
katana-executor = { .., features = [ "native" ] }
```

and the following needs to be setup:

LLVM 17 needs to be installed and the `MLIR_SYS_170_PREFIX` and `TABLEGEN_170_PREFIX` environment variable needs to point to said installation.

In macOS, run

```console
brew install llvm@17
```

and export the following environment variables:

```bash
export MLIR_SYS_170_PREFIX=/opt/homebrew/opt/llvm@17
export LLVM_SYS_170_PREFIX=/opt/homebrew/opt/llvm@17
export TABLEGEN_170_PREFIX=/opt/homebrew/opt/llvm@17
```

and you're set.
