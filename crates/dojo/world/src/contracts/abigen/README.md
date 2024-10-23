# Embedded ABI for contracts

To ease the re-use of `dojo-world` crate on other projects that are not aware of the whole dojo stack, the ABI used for binding generation are decoupled from the compilation process.

To generate the ABI in `world.rs` or `model.rs`, please consider to run:

```bash
cargo run -p dojo-world-abigen -r
```

The CI runs the same command with the `--check` argument, to ensure that the ABI that are inside the rust modules are still consistent with the latest version of `dojo-core` contracts.
