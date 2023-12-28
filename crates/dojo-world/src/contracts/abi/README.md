# Embedded ABI for contracts

Currently, the ABIs for `world` and `executor` are embedded in the repo.
To build them, consider the following:

1. Change directory into `examples/spawn-and-move` at the root of the workspace.
2. Build the example with `sozo`.
3. Extract the ABI key only for `world` and `executor`:
```
sozo build
jq .abi ./target/dev/dojo\:\:world\:\:world.json > ../../crates/dojo-world/src/contracts/abi/world.json
jq .abi ./target/dev/dojo\:\:executor\:\:executor.json > ../../crates/dojo-world/src/contracts/abi/executor.json
```
4. Copy and paste the ABI in the rust module, this avoids path issue when crates are used in third party project.

In the future, Cainome will have a CLI tool to auto-generate those files with one command.
