# Indexer

Before running the indexer, you will need to generate the database client interface. To do so, you can run `generate` on the prisma cli

`cargo run --bin prisma-cli generate`

When you are ready to run the indexer, you can start it by doing `cargo run --bin dojo-index` and supplying the world address you're going to be indexing and an rpc.

`cargo run --bin dojo-index <WORLD> <RPC>`
