# Development Setup

This guide outlines the steps for setting up an environment for developing the Dojo codebase. If you want to use Dojo to make things, follow the [Dojo Installation guide](https://book.dojoengine.org/getting-started) instead.

## Repository Structure

After recent architectural improvements, the Dojo ecosystem consists of several repositories:

- **This Repository (dojoengine/dojo)**: Core Dojo framework, Sozo CLI, ECS components, and smart contracts
- **Katana Repository** ([dojoengine/katana](https://github.com/dojoengine/katana)): High-performance sequencer for Dojo
- **Torii Repository** ([dojoengine/torii](https://github.com/dojoengine/torii)): Automatic indexer and GraphQL/gRPC API

## System Prerequisites

- **Rust** (latest stable version)
- **Cairo** (via dojoup)  
- **Katana** (installed separately - see below)
- **Torii** (installed separately - see below)
- **protoc** (Protocol Buffers compiler)
- **Optional**: VS Code extension for syntax highlighting

See the [Dojo Installation guide](https://book.dojoengine.org/getting-started) for more details on installing these dependencies.

### Installing External Components

Since Katana and Torii are now in separate repositories, you have two options:

#### Option 1: Install via dojoup (Recommended)
```bash
curl -L https://install.dojoengine.org | bash
dojoup
```

#### Option 2: Install from source
```bash
# Install Katana from source
git clone https://github.com/dojoengine/katana.git
cd katana
cargo install --path . --locked --force

# Install Torii from source  
git clone https://github.com/dojoengine/torii.git
cd torii
cargo install --path . --locked --force
```

## Setting Up Your Environment

### 1. Clone the Repository

```bash
git clone https://github.com/dojoengine/dojo.git && cd dojo
```

### 2. Architecture Overview

At the top level, this repository is organized as follows:

- **crates/**: Source code of the different libraries that make up core Dojo
- **bin/**: Source code of the different binaries (primarily Sozo)
- **examples/**: Simple Dojo projects for testing and reference
- **scripts/**: Useful scripts for developers and CI

Key components in this repository:
- **sozo**: The contract manager and Dojo compiler
- **dojo/core**: The core contracts written in Cairo
- **dojo/lang**: The Dojo plugin for the Cairo compiler

**Note**: Katana and Torii are now maintained in separate repositories:
- Katana: https://github.com/dojoengine/katana
- Torii: https://github.com/dojoengine/torii

### 3. Run the Tests

We use `nextest` as our test runner. Install it from https://nexte.st/.

Note that tests depend on database artifacts for a simple "spawn and move" game:

```bash
# Prepare the spawn-and-move db artifact
bash scripts/extract_test_db.sh

# Run all tests
cargo nextest run --all-features --workspace

# Run a single package test
cargo nextest run --all-features -p sozo-ops
```

## Testing Your Changes

Before submitting a pull request, run the tests locally to ensure your changes work correctly:

```bash
# Run the same command that CI uses
cargo nextest run --all-features --workspace --build-jobs 20
```

The built-in Continuous Integration (CI) will run all tests when you push changes. Test results appear in your pull request's GitHub interface.

## Rebuilding Artifacts

If you modified the `dojo-core` or `dojo-lang` crates, you must rebuild the database artifacts.

### Method 1: Using System Katana
If you have a compatible Katana version in your `$PATH`:

```bash
bash scripts/rebuild_test_artifacts.sh
```

If you see "No version is set for command scarb", run `asdf current` to check versions.

### Method 2: Build Katana from Source
If you need a specific Katana version:

```bash
# In a new terminal
git clone https://github.com/dojoengine/katana.git && cd katana
cargo build --bin katana -r
cp target/release/katana /tmp/
```

Then run the rebuild script from the Dojo directory.

**Note**: Dojo looks for test dependencies (katana binary and spawn-and-move artifact) in `/tmp/` by convention.

## Development Workflow

### Working with Core Components
For changes to ECS framework, smart contracts, or Sozo:

```bash
# Make changes to relevant crates
# Build and test
cargo build --all
cargo nextest run --all-features --workspace

# Test with examples
cd examples/spawn-and-move
sozo build
```

### Full Stack Development
For end-to-end testing with the complete Dojo stack:

```bash
# Terminal 1: Start Katana
katana --dev

# Terminal 2: Build and deploy your world  
cd examples/spawn-and-move
sozo build && sozo migrate

# Terminal 3: Start Torii indexer
torii --world <WORLD_ADDRESS>

# Terminal 4: Run your client application (if applicable)
cd client && yarn dev
```

### Contributing to Related Repositories
- **Katana issues**: [dojoengine/katana](https://github.com/dojoengine/katana)
- **Torii issues**: [dojoengine/torii](https://github.com/dojoengine/torii)  
- **Client SDK issues**: [dojoengine/dojo.js](https://github.com/dojoengine/dojo.js)
- **Documentation**: [dojoengine/book](https://github.com/dojoengine/book)

## Releasing

Propose a new release by manually triggering the `release-dispatch` GitHub action. The version value can be semver or a level: `[patch, minor, major]`.

Once run, the workflow creates a PR with the versioned repo, which triggers the release flow and creates a draft release on merge.

## Getting Help

- **Documentation**: [Dojo Book](https://book.dojoengine.org)
- **Discord**: [Dojo Community Discord](https://discord.gg/dojoengine)  
- **Issues**: Create issues in the appropriate repository based on the component