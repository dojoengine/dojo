# Simple ticking counter

The `CounterTarget` contract implements the ticking interface and will increment its counter for each new block.
Each time the `tick` method is called a `Tick` event is emitted.

## Setup
To compile, declare and deploy and set ticker contract target, just run: `make`

[starkli](https://book.starkli.rs/) is required by setup script

## Account configuration
[`depositor`](./config/depositor.json) and [`operator`](./config/operator.json) starkli json files need to be updated if account class hash or seed is updated.

`depositor` and `operator` private key are hardcoded in [setup.bash](./scripts/setup.bash)
