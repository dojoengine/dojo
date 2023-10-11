# This crate is dedicated for benchmarking purposes

## Prerequisites

-   cargo - for test case generation and runtime
-   katana - as a local RPC server
-   sozo - for contract compilation and deployment

## Running benchmarks

While benchmarks are running Katana instance has to be online.

```bash
katana --disable-fee
```

Then contracts have to be built and deployed.

```bash
sozo --manifest-path crates/benches/contracts/Scarb.toml build
sozo --manifest-path crates/benches/contracts/Scarb.toml migrate
```

Last command should print a _world address_ that should be pasted into `default_auth.sh` file, and then authorize with `bash crates/benches/contracts/scripts/default_auth.sh`

## Running with compiled `sozo`

While during testing release version of the tool worked better, Sozo can be run from source with

```bash
cargo run -r --bin sozo -- --manifest-path crates/benches/contracts/Scarb.toml build
```
