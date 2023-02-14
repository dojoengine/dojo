# dojo-lang

Cairo language plugin for compiling the Dojo Entity Component System to Starknet contracts.

## Testing

Expected test outputs are defined in `crates/dojo-lang/src/plugin_test_data/component`.

To run the tests, run:

```
cargo test --package dojo-lang --lib -- plugin::test::expand_contract::component --exact --nocapture
```

To regenerate, set `CAIRO_FIX_TESTS=1`.
