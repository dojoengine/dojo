# Embedded ABI for contracts

Currently, the ABIs for `world` and `executor` are embedded in the repo.
To build them, consider the following:

1. Change directory into `examples/spawn-and-move` at the root of the workspace.
2. Build the example with `sozo`.
3. Extract the ABI key only for `world` and `executor`:
```
sozo build
jq .abi ./target/dev/dojo_example-world.json > crates/dojo-world/src/contract/abi/world.json
jq .abi ./target/dev/dojo_example-executor.json > crates/dojo-world/src/contract/abi/executor.json
```
