#!/bin/bash
set -o pipefail
set -e

rm -f indexer.db
cargo sqlx database create --database-url sqlite://$PWD/indexer.db
cargo sqlx migrate run --database-url sqlite://$PWD/indexer.db
cargo sqlx prepare --database-url sqlite://$PWD/indexer.db