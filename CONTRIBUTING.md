# Contributing to Dojo

Thank you for considering contributing to Dojo. It's people like you that make Dojo such a great project and community.

Please follow the following guidelines in order to streamline your contribution. It helps communicate that you respect the time of the developers maintaining this open source project. In return, maintainers will reciprocate that respect when addressing your issue, assessing your changes, and helping you finalize your pull requests.

### Check existing issues

Before you start contributing, please check the [Issue Tracker](https://github.com/dojoengine/dojo/issues) to see if there are any existing issues that match what you are intending to do. If the issue doesn't exist, please create it.

If you are creating a new issue, please provide a descriptive title and detailed description. If possible, include a code sample or an executable test case demonstrating the expected behavior that is not occurring.

### Fork and Clone the Repository

Once you've found an issue to work on, the next step is to fork the Dojo repo and clone it to your local machine. This is necessary because you probably won't have push access to the main repo.

### Setting up your environment

You will need the [Rust](https://rust-lang.org) compiler and Cargo, the Rust package manager.
The easiest way to install both is with [`rustup.rs`](https://rustup.rs/).

On Windows, you will also need a recent version of [Visual Studio](https://visualstudio.microsoft.com/downloads/),
installed with the "Desktop Development With C++" Workloads option.

## Architecture

At the top level, dojo is composed of different folders:

- [`crates`](crates/): This folder contains the source code of the different crates (which are libraries) that make up Dojo.
- [`bin`](bin/): This folder contains the source code of the different binaries that make up Dojo.
- [`examples`](examples/): This folder contains the source code of simple Dojo projects that can be used as a starting point for new projects and also useful for testing.
- [`scripts`](scripts/): A set of useful scripts for developers and CI.

Inside `bin` and `crates` you will find source code related to Dojo stack components:

- `katana`: The Starknet sequencer tailored for gaming.
- `sozo`: The contract manager and Dojo compiler.
- `torii`: The indexer that store the state of your World.
- `dojo/core`: The core contract of Dojo written in Cairo.
- `dojo/lang`: The Dojo plugin for the Cairo compiler.

It is important to note that `bin` should only contain applications that gathers user inputs and delegates the work to the libraries present into the crates.

As an example, `sozo` is a CLI that gathers user inputs and delegates the work to run the commands code to the `sozo` crate.

## Making Changes

When you're ready to start coding, create a new branch on your cloned repo. It's important to use a separate branch for each issue you're working on. This keeps your changes separate in case you want to submit more than one contribution.

Please use meaningful names for your branches. For example, if you're working on a bug with the ECS, you might name your branch `fix-ecs-bug`.

As you're making changes, make sure you follow the coding conventions used throughout the Dojo project. Consistent code style makes it easier for others to read and understand your code.

## Testing the changes

To speed the test suite and avoid migrating dojo projects again, `katana` databases are compressed and stored in the repo.

If you don't have any change in the `dojo/core` crate or any cairo example, you only have to extract the databases:

```bash
bash scripts/extract_test_db.sh
```

To test your changes, if you have modified the `dojo/core` crate or any cairo example, you will need to regenerate the databases:

```bash
# Prints the policies to then be copied into the `sozo/tests/test_data/policies.json` test file to ensure entrypoints and addresses are up to date.
POLICIES_FIX=1 cargo nextest run --all-features --build-jobs 20 --workspace --nocapture policies

# Ensures the test databases are up to date.
bash scripts/rebuild_test_artifacts.sh
```

Then you can run the tests:

```bash
# If you don't have any change in Katana:
cargo nextest run --all-features --build-jobs 20 --workspace

# If you have changes in Katana, you must use local Katana to test.
cargo build -r --bin katana
KATANA_RUNNER_BIN=./target/release/katana cargo nextest run --all-features --build-jobs 20 --workspace
```

## Submitting a Pull Request

Once your changes are ready, commit them and push the branch to your forked repo on GitHub. Then you can open a pull request from your branch to the `main` branch of the Dojo repo.

When you submit the pull request, please provide a clear, detailed description of the changes you've made. If you're addressing a specific issue, make sure you reference it in the description.

Your pull request will be reviewed by the maintainers of the Dojo project. They may ask for changes or clarification on certain points. Please address their comments and commit any required changes to the same branch on your repo.

## Documentation

We strive to provide comprehensive, up-to-date documentation for Dojo. If your changes require updates to the documentation, please include those in your pull request.

The [Dojo Book repository](https://github.com/dojoengine/book) is where you should submit your changes to the documentation.

## Final Notes

Again, thank you for considering to contribute to Dojo. Your contribution is invaluable to us. We hope this guide makes the contribution process clear and answers any questions you might have. If not, feel free to ask on the [Discord](https://discord.gg/PwDa2mKhR4) or on [GitHub](https://github.com/dojoengine/dojo/issues).
