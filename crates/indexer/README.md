# Indexer

Before running the indexer, if any changes were made to the prisma schema file, you will need to regenerate the database client interface. To do so, you can run `generate` on the prisma cli

`cargo prisma generate`

When you are ready to run the indexer, you can start it by doing `cargo run --bin dojo-index` and supplying the world address you're going to be indexing and an rpc.

`cargo run --bin dojo-index <WORLD> <RPC>`