#!/bin/bash
set -e

pushd $(dirname "$0")/..

yes | cargo sqlx database reset --database-url sqlite://$PWD/indexer.db
cargo sqlx migrate run --database-url sqlite://$PWD/indexer.db
cargo sqlx prepare --database-url sqlite://$PWD/indexer.db