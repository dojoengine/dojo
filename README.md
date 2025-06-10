![Dojo Feature Matrix](.github/feature_matrix.png)

# Dojo: Provable Games and Applications [![discord](https://img.shields.io/badge/join-dojo-green?logo=discord&logoColor=white)](https://discord.com/invite/dojoengine) [![Telegram Chat][tg-badge]][tg-url] [![Github Actions][gha-badge]][gha-url]

[gha-badge]: https://img.shields.io/github/actions/workflow/status/dojoengine/dojo/ci.yml?branch=main
[gha-url]: https://github.com/dojoengine/dojo/actions/workflows/ci.yml?query=branch%3Amain
[tg-badge]: https://img.shields.io/endpoint?color=neon&logo=telegram&label=chat&style=flat-square&url=https%3A%2F%2Ftg.sumanjay.workers.dev%2Fdojoengine
[tg-url]: https://t.me/dojoengine

Dojo is a developer friendly framework for building **provable** Games, Autonomous Worlds and other Applications that are natively composable, extensible, permissionless and persistent. It is an extension of [Cairo](https://www.cairo-lang.org/), an efficiently provable language, that supports generation of zero-knowledge proofs attesting to a computations validity and enables exponential scaling of onchain computation while maintaining the security properties of Ethereum.

It is designed to significantly reduce the complexity of developing provable applications that can be deployed to and verified by blockchains. It does so by providing a ~zero-cost abstraction for developers to succinctly define provable applications and a robust toolchain for building, migrating, deploying, proving and settling these worlds in production.

## Getting Started

See the [getting started](https://book.dojoengine.org/tutorials/dojo-starter) section in the Dojo book to start building provable applications with Dojo.

You can find more detailed documentation in the Dojo Book [here](https://book.dojoengine.org/).

## Development

We welcome contributions of all kinds from anyone. See our [Development](/DEVELOPMENT.md) and [Contributing](/CONTRIBUTING.md) guides for more information on setting up your developer environment and how to get involved.

If you encounter issues or have questions, you can [submit an issue on GitHub](https://github.com/dojoengine/dojo/issues). You can also join our [Discord](https://discord.gg/dojoengine) for discussion and help.

## Built with Dojo

- [Awesome Dojo](https://github.com/dojoengine/awesome-dojo)
- [Origami](https://github.com/dojoengine/origami)

## Audit

Dojo core smart contracts have been audited:

- Feb-24: [Nethermind Security](https://github.com/NethermindEth/PublicAuditReports/blob/main/NM0159-FINAL_DOJO.pdf).
- Nov-24: [OpenZeppelin](https://blog.openzeppelin.com/dojo-security-review) and it's [diff-audit](https://blog.openzeppelin.com/dojo-namespace-diff-audit).
