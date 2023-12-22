# This crate is dedicated for benchmarking purposes

## Quick start

```bash
katana
bash scripts/cargo_bench.sh
```

## Prerequisites

-   `cargo` - for test case generation and runtime
-   `katana` - as a local RPC server
-   `sozo` - for contract compilation and deployment

## Requirements for running

While benchmarks are running a Katana instance has to be online either remotely or locally...

```bash
katana
```

...contracts have to be built and deployed...

```bash
sozo --manifest-path crates/benches/contracts/Scarb.toml build
sozo --manifest-path crates/benches/contracts/Scarb.toml migrate
```

...and actions authorized.

```bash
crates/benches/contracts/scripts/default_auth.sh
```

Then tests can be run with

```bash
cargo test bench -- --ignored
```

Benchmarks are ignored by default because they need a while to complete and need a running Katana. Their names should start with bench.

## Running with compiled `sozo`

While during testing release version of the tool worked better, Sozo can be run from source with

```bash
cargo run -r --bin sozo -- --manifest-path crates/benches/contracts/Scarb.toml build
```
