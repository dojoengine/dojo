# Saya executable

Saya executable runs Saya service with CLI configurations.

Currently, Saya fetches some Katana blocks, and can publish the state update on a Celestia node.

Example:
```bash
cargo run --bin saya -- \
    --rpc-url http://localhost:5050 \
    --da-chain celestia \
    --celestia-node-url http://127.0.0.1:26658 \
    --celestia-namespace mynm \
    --celestia-node-auth-token eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.....
```

## WIP

1. Enhance how options are used for each one of the possible DA.
2. Add a all-in-one toml file to configure the whole Saya service (DA, prover, etc...) to avoid a huge command line.
3. Add subcommands along with saya development for a better experience.
