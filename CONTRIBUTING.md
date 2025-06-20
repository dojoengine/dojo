# Contributing to Dojo

Thank you for considering contributing to Dojo. It's people like you that make Dojo such a great project and community.

Please follow the following guidelines in order to streamline your contribution. It helps communicate that you respect the time of the developers maintaining this open source project. In return, maintainers will reciprocate that respect when addressing your issue, assessing your changes, and helping you finalize your pull requests.

## Repository Architecture

The Dojo ecosystem is organized across multiple repositories to improve maintainability and development efficiency:

| Component | Repository | Purpose |
|-----------|------------|---------|
| **Core Framework** | [dojoengine/dojo](https://github.com/dojoengine/dojo) | ECS framework, Sozo CLI, smart contracts |
| **Katana** | [dojoengine/katana](https://github.com/dojoengine/katana) | High-performance sequencer |
| **Torii** | [dojoengine/torii](https://github.com/dojoengine/torii) | Automatic indexer and API layer |
| **Client SDKs** | [dojoengine/dojo.js](https://github.com/dojoengine/dojo.js) | JavaScript/TypeScript SDK |
| **Documentation** | [dojoengine/book](https://github.com/dojoengine/book) | Official documentation |

## Which Repository Should I Use?

### Core Framework (This Repository)
Contribute here for:
- ✅ ECS framework improvements
- ✅ Sozo CLI features and bug fixes  
- ✅ Smart contract templates and examples
- ✅ Core compilation and build tools
- ✅ Integration issues between components

### External Repositories
| Type of Contribution | Repository | Examples |
|---------------------|------------|----------|
| Sequencer issues | [katana](https://github.com/dojoengine/katana) | Block production, RPC endpoints, performance |
| Indexing/API issues | [torii](https://github.com/dojoengine/torii) | GraphQL queries, event indexing, subscriptions |
| Client SDK issues | [dojo.js](https://github.com/dojoengine/dojo.js) | JavaScript SDK, React hooks, TypeScript types |
| Documentation | [book](https://github.com/dojoengine/book) | Tutorials, guides, API documentation |

## Check Existing Issues

Before you start contributing, please check the Issue Tracker to see if there are any existing issues that match what you are intending to do. If the issue doesn't exist, please create it.

If you are creating a new issue, please provide a descriptive title and detailed description. If possible, include a code sample or an executable test case demonstrating the expected behavior that is not occurring.

### Issue Reporting Guidelines

| Issue Type | Repository | Examples |
|------------|------------|----------|
| Framework bugs | [dojo](https://github.com/dojoengine/dojo/issues) | ECS bugs, Sozo crashes, compilation errors |
| Performance issues | [katana](https://github.com/dojoengine/katana/issues) | Slow block production, RPC timeouts |
| Query problems | [torii](https://github.com/dojoengine/torii/issues) | GraphQL errors, missing events, sync issues |
| SDK issues | [dojo.js](https://github.com/dojoengine/dojo.js/issues) | TypeScript errors, React hooks, integration |
| Documentation gaps | [book](https://github.com/dojoengine/book/issues) | Missing guides, unclear examples |

## Fork and Clone the Repository

Once you've found an issue to work on, the next step is to fork the appropriate repository and clone it to your local machine. This is necessary because you probably won't have push access to the main repo.

## Setting Up Your Environment

You will need the Rust compiler and Cargo, the Rust package manager. The easiest way to install both is with [rustup.rs](https://rustup.rs/).

On Windows, you will also need a recent version of Visual Studio, installed with the "Desktop Development With C++" Workloads option.

See our [Development Guide](./DEVELOPMENT.md) for detailed setup instructions.

## Architecture

At the top level, this repository is composed of different folders:

- **crates**: Source code of the different libraries that make up core Dojo
- **bin**: Source code of the different binaries (primarily Sozo)  
- **examples**: Simple Dojo projects for testing and reference
- **scripts**: Useful scripts for developers and CI

Inside bin and crates you will find source code related to core Dojo components:

- **sozo**: The contract manager and Dojo compiler
- **dojo/core**: The core contracts written in Cairo
- **dojo/lang**: The Dojo plugin for the Cairo compiler

**Important**: The following components are now maintained in separate repositories:
- **katana**: The Starknet sequencer (now at [dojoengine/katana](https://github.com/dojoengine/katana))
- **torii**: The indexer that stores World state (now at [dojoengine/torii](https://github.com/dojoengine/torii))

It is important to note that `bin` should only contain applications that gather user inputs and delegate work to the libraries present in `crates`. As an example, `sozo` is a CLI that gathers user inputs and delegates the work to run commands to the `sozo` crate.

## Making Changes

When you're ready to start coding, create a new branch on your cloned repo. It's important to use a separate branch for each issue you're working on. This keeps your changes separate in case you want to submit more than one contribution.

Please use meaningful names for your branches. For example, if you're working on a bug with the ECS, you might name your branch `fix-ecs-bug`.

As you're making changes, make sure you follow the coding conventions used throughout the Dojo project. Consistent code style makes it easier for others to read and understand your code.

## Testing the Changes

To speed the test suite and avoid migrating dojo projects again, katana databases are compressed and stored in the repo.

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

# If you have changes that require testing with Katana, you must use a local Katana binary.
# First, build or obtain a compatible Katana binary (see DEVELOPMENT.md for details)
cargo build -r --bin katana  # This will fail if Katana is not in this repo
# Use system Katana or build from separate repository instead
KATANA_RUNNER_BIN=katana cargo nextest run --all-features --build-jobs 20 --workspace
```

**Note**: Since Katana is now in a separate repository, you'll need to install it separately or build it from the [Katana repository](https://github.com/dojoengine/katana) if tests require it.

## Submitting a Pull Request

Once your changes are ready, commit them and push the branch to your forked repo on GitHub. Then you can open a pull request from your branch to the main branch of the Dojo repo.

When you submit the pull request, please provide a clear, detailed description of the changes you've made. If you're addressing a specific issue, make sure you reference it in the description.

Your pull request will be reviewed by the maintainers of the Dojo project. They may ask for changes or clarification on certain points. Please address their comments and commit any required changes to the same branch on your repo.

## Cross-Repository Contributions

Some features may require changes across multiple repositories:

### Example: Adding a New ECS Feature
1. **Core Framework** (this repo): Implement the ECS feature
2. **Katana** (if needed): Add sequencer support for new events  
3. **Torii** (if needed): Add indexing for new data structures
4. **dojo.js** (if needed): Add client SDK support
5. **Documentation**: Update guides and examples

### Coordination Process
1. Create issues in each relevant repository
2. Reference cross-repository issues in descriptions
3. Coordinate with maintainers for release planning
4. Test integration between components

## Documentation

We strive to provide comprehensive, up-to-date documentation for Dojo. If your changes require updates to the documentation, please include those in your pull request.

The [Dojo Book repository](https://github.com/dojoengine/book) is where you should submit your changes to user-facing documentation. 
## Final Notes

Again, thank you for considering contributing to Dojo. Your contribution is invaluable to us. We hope this guide makes the contribution process clear and answers any questions you might have.

If you have questions about:
- **Which repository to use**: Ask in our [Discord #dev-help channel](https://discord.gg/dojoengine)
- **Technical approach**: Create a discussion issue before coding
- **Contribution scope**: Reach out to maintainers

For more detailed development setup instructions, see our [Development Guide](./DEVELOPMENT.md).

## Community and Communication

### Getting Help
- **Discord**: Join our [Discord community](https://discord.gg/dojoengine) for real-time discussions
- **GitHub Discussions**: Use GitHub Discussions for longer-form questions
- **Documentation**: Check the [Dojo Book](https://book.dojoengine.org) for comprehensive guides

### Recognition

Contributors are recognized through:
- **GitHub**: Automatic contribution tracking
- **Discord**: Special contributor roles  
- **Documentation**: Major contributors listed in project documentation

Thank you for contributing to Dojo! 