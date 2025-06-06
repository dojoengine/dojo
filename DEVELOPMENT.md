# Development Setup

This guide outlines the steps for setting up a development environment for Dojo itself.
If you want to use Dojo to make things, follow the [Dojo Installation guide](https://book.dojoengine.org/installation) instead.

## System prerequisites

- [Rust](https://github.com/rust-lang/rust)
- [Cairo](https://github.com/starkware-libs/cairo)
- [protoc](https://github.com/protocolbuffers/protobuf)
- Optional: [VS Code extension](https://marketplace.visualstudio.com/items?itemName=starkware.cairo1)

See the [Dojo Installation guide](https://book.dojoengine.org/installation) for more details on installing these dependencies.

> Depending on your Linux distribution, you may need to install additional dependencies.

## Setting up your environment

### 1. Clone the repository

```sh
# Clone and enter the repo
git clone https://github.com/dojoengine/dojo.git
cd dojo
```

### 2. Run the tests

We're using `nextest` as our test runner, which you can get at [https://nexte.st/](https://nexte.st/).

> Note that the tests depend on database artifacts for a simple "spawn and move" game.

```sh
# Prepare the spawn-and-move db artifact
bash scripts/extract_test_db.sh

# Run all the tests
cargo nextest run --all-features --workspace

# Run a single package test
cargo nextest run --all-features -p sozo-ops
```

By convention, Dojo stores test dependencies in your system's `/tmp/` directory.

## Testing your changes

Before you submit your pull request, you should run the tests locally to make sure your changes haven't broken anything.
You can execute the same command that will be executed on the CI by checking the [`.github/workflows/test.yml`](.github/workflows/test.yml) file.

When you push your changes, the built-in Continuous Integration (CI) will run all the tests on your new code.
You can see the result of these tests in the GitHub interface of your pull request.
If the tests fail, you'll need to revise your code and push it again.

> The CI uses a `devcontainer` to have all the dependencies installed and to run the tests.
> You can find more information about the devcontainer in the [`.devcontainer.json`](.devcontainer/devcontainer.json) file and see the latest releases on [GitHub package](https://github.com/dojoengine/dojo/pkgs/container/dojo-dev).

### Rebuilding artifacts

If you modified the `dojo-core` or `dojo-lang` crates you must rebuild the db artifacts.
This will require a compatible version of Katana.

If you have a compatible version of Katana in your `$PATH`, simply run the following command:

```bash
# Rebuild the spawn-and-move db artifact
bash scripts/rebuild_test_artifacts.sh
```

> If you receive error messages saying `No version is set for command scarb`, run `asdf current` to check your installed versions.

Otherwise, you will need to build Katana from source and copy it to the `/tmp/` directory.
In a new terminal window, run:

```sh
# Clone and enter the repo
git clone https://github.com/dojoengine/katana.git
cd katana

# Build a new katana binary from source
cargo build --bin katana -r

# Copy the binary to the /tmp/ directory
cp target/release/katana /tmp/
```

Then you can run the `rebuild_test_artifacts` script from the Dojo directory.

> Note: Katana depends on [Bun](https://bun.sh/) for development, which you will need to install.
> For more information, see [the Katana README](https://github.com/dojoengine/katana).

## Releasing

Propose a new release by manually triggering the `release-dispatch` github action. The version value can be an semver or a level: `[patch, minor, major]`.

Once run, the workflow will create a PR with the versioned repo which will trigger the release flow and the creation of a draft release on merge.
