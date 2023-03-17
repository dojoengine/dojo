# Dojo Indexer

Dojo Indexer is a command line tool that allows you to index data from a given Dōjō world using Apibara and an RPC endpoint while storing indexed data such as the world's components, entity states and systems, in a specified database.


## Prerequisites
- Before running the indexer you will need the `sqlx-cli`:
    ```
    cargo install sqlx-cli
    ```
- Create an SQLite database using `sqlx`:

  1. Set the database URL that will be used by `sqlx`:

     ```
     export DATABASE_URL=sqlite://indexer.db
     ```

  2. Create a database file at that URL:

     ```
     cargo sqlx database create
     ```

  3. Run the migrations:

     ```
     cargo sqlx migrate run
     ```

## Usage

To run the `dojo-indexer` command, open your terminal or command prompt, navigate to the directory containing the Dojo Indexer source code, and execute the following command:

```
cargo run --bin dojo-indexer <world> <node> <rpc> <database_url>
```

- `<world>`: The address of the world you want to index.
- `<node>`: The Apibara node you want to use for indexing.
- `<rpc>`: The RPC endpoint of your starknet node.
- `<database_url>`: The URL of the database where you want to store the indexed data.