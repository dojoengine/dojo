# Contributing to Dojo

First of all, thank you for considering contributing to Dojo. It's people like you that make Dojo such a great tool.

Following these guidelines helps to communicate that you respect the time of the developers managing and developing this open source project. In return, they should reciprocate that respect in addressing your issue, assessing changes, and helping you finalize your pull requests.

## Getting Started

### Check the Issues

Before you start contributing, please check the [Issue Tracker](https://github.com/dojoengine/dojo/issues) to see if there are any existing issues that match what you're intending to do. If the issue doesn't exist, please create it.

If you're creating a new issue, please provide a descriptive title and detailed description. If possible, include a code sample or an executable test case demonstrating the expected behavior that is not occurring.

### Fork and Clone the Repository

Once you've found an issue to work on, the next step is to fork the Dojo repo and clone it to your local machine. This is necessary because you probably won't have push access to the main repo.

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
- `dojo-core`: The core contract of Dojo written in Cairo.
- `dojo-lang`: The Dojo plugin for the Cairo compiler.

It is important to note that `bin` should only contain applications that gathers user inputs and delegates the work to the libraries present into the crates.

As an example, `sozo` is a CLI that gathers user inputs and delegates the work to run the commands code to the `sozo` crate.

## Making Changes

When you're ready to start coding, create a new branch on your cloned repo. It's important to use a separate branch for each issue you're working on. This keeps your changes separate in case you want to submit more than one contribution.

Please use meaningful names for your branches. For example, if you're working on a bug with the ECS, you might name your branch `fix-ecs-bug`.

As you're making changes, make sure you follow the coding conventions used throughout the Dojo project. Consistent code style makes it easier for others to read and understand your code.

## Submitting a Pull Request

Once your changes are ready, commit them and push the branch to your forked repo on GitHub. Then you can open a pull request from your branch to the `main` branch of the Dojo repo.

When you submit the pull request, please provide a clear, detailed description of the changes you've made. If you're addressing a specific issue, make sure you reference it in the description.

Your pull request will be reviewed by the maintainers of the Dojo project. They may ask for changes or clarification on certain points. Please address their comments and commit any required changes to the same branch on your repo.

## Running Tests

Before you submit your pull request, you should run the test suite locally to make sure your changes haven't broken anything.

To run the test, you can execute the same command that will be exected on the CI by checking the [`.github/workflows/ci.yml`](.github/workflows/ci.yml) file.

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

## Documentation

We strive to provide comprehensive, up-to-date documentation for Dojo. If your changes require updates to the documentation, please include those in your pull request.

The [dojo book repository](https://github.com/dojoengine/book) is where you should submit your changes to the documentation.

## Final Notes

Again, thank you for considering to contribute to Dojo. Your contribution is invaluable to us. We hope this guide makes the contribution process clear and answers any questions you might have. If not, feel free to ask on the [Discord](https://discord.gg/PwDa2mKhR4) or on [GitHub](https://github.com/dojoengine/dojo/issues).
