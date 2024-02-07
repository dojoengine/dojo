# This crate is dedicated for benchmarking purposes

Due to long times, and unreliable nature benchmarks are not run in the CI env at this time.
The benchmarks themselfs are hidden behind features not to run with `--all-features`.
To run benchmarks one can use `cargo test --no-default-features`

## Prerequisites

-   `cargo` - for test case generation and runtime
-   `katana` - as a local RPC server
-   `sozo` - for contract compilation and deployment


## Katana Benchmarks

TMP: Due to the number of accounts overfowing `u8` at the moment, katana binary is run from `target` directory.
After the change to `u16` is released it should take the system installed version. Because of this additional first step is required.

```bash
cargo build -r --bin katana
```

And to run the benchamarks 

```bash
cargo test --manifest-path crates/benches/Cargo.toml -- --nocapture
```

## Gas usage Benchmarks

### Quick start

```bash
katana
bash scripts/cargo_bench.sh
```


### Requirements for running

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

### Running with compiled `sozo`

While during testing release version of the tool worked better, Sozo can be run from source with

```bash
cargo run -r --bin sozo -- --manifest-path crates/benches/contracts/Scarb.toml build
```
