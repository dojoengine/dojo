# Cairo proc macros for the Dojo framework.

This crate contains the proc macros for the Dojo framework.

## List of macros

### Attribute macros
- `#[dojo::model]`: Defines a struct as a model.
- `#[dojo::event]`: Defines a struct as an event.
- `#[dojo::contract]`: Defines a struct as a contract.

### Derive macros
- `#[derive(Introspect)]`: Makes the struct introspectable, which allows to get the struct metadata at runtime and for offchain components.
- `#[derive(IntrospectPacked)]`: Same as `#[derive(Introspect)]` but use this one if you wish your struct to be packed in the storage (usually uses less space).
- `#[derive(DojoStore)]`: Derives `DojoStore` trait for the struct, which allows to store the struct in the world's database.
- `#[derive(DojoLegacyStore)]`: Uses the legacy storage API for the struct, only for models that were deployed in a world on mainnet previous to `1.7.0`.

More information about the migration to `1.7.0` can be found [in the book](https://book.dojoengine.org/framework/upgrading/dojo-1-7).

## Usage

Add the following to your `Scarb.toml` file:

```toml
[dependencies]
dojo_cairo_macros = "1.7.0"
```
