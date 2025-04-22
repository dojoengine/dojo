# Development Setup

This guide outlines the steps to set up a development environment for Dojo. If you just want to play with the toolchain, we recommend following the [Quick Start](/getting-started.md) guide instead.

## Prerequisites

- [Rust](https://github.com/rust-lang/rust)
- [Cairo](https://github.com/starkware-libs/cairo)
- [protoc](https://github.com/protocolbuffers/protobuf)

## Setup

### 1. Clone the repository

```sh
git clone https://github.com/dojoengine/dojo.git
```

### 2. Install Rust and dependencies

Install & update Rust:

```sh
rustup override set stable && rustup update
```

Now run the test suite to confirm your setup:

```sh
# First generate db artifacts for the tests
bash scripts/extract_test_db.sh

# The run the tests
cargo test
```

Note: Depending on your Linux distribution, you may need to install additional dependencies.

### 3. Install Scarb package manager

Install the [Scarb](https://docs.swmansion.com/scarb).

### 4. Add the Cairo 1.0 VSCode extension

Install the [Cairo 1.0](https://marketplace.visualstudio.com/items?itemName=starkware.cairo1) extension for Visual Studio Code.

## Testing

Before you submit your pull request, you should run the test suite locally to make sure your changes haven't broken anything.

We're using `nextest` as our test runner, which you can get at [https://nexte.st/](https://nexte.st/).

To run the test, you can execute the same command that will be executed on the CI by checking the [`.github/workflows/ci.yml`](.github/workflows/ci.yml) file.

```bash
# Run all the tests excluding Katana (due to SiR dependency, they may be run independently)
cargo nextest run --all-features --workspace --exclude katana

# To limit the resources, you can run the tests only on a package:
cargo nextest run --all-features -p sozo-ops
```

If you have to modify `dojo-core` or `dojo-lang` crates you must:

```bash
# First spin up a **fresh** Katana instance on default port.
cargo run --bin katana

# Then execute the script that will rebuild them.
bash scripts/rebuild_test_artifacts.sh
```

Additionally, when you push your changes, the built-in Continuous Integration (CI) will also run all the tests on the pushed code. You can see the result of these tests in the GitHub interface of your pull request. If the tests fail, you'll need to revise your code and push it again.

The CI uses a `devcontainer` to have all the dependencies installed and to run the tests. You can find more information about the devcontainer in the [`.devcontainer.json`](.devcontainer/devcontainer.json) file and see the latest releases on [GitHub package](https://github.com/dojoengine/dojo/pkgs/container/dojo-dev).

## Releasing

Propose a new release by manually triggering the `release-dispatch` github action. The version value can be an semver or a level: `[patch, minor, major]`.

Once run, the workflow will create a PR with the versioned repo which will trigger the release flow and the creation of a draft release on merge.
