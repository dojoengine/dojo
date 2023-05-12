#!/bin/bash
set -o pipefail
set -e

# if error 'unable to open database file', 
# use absolute path to database file
# https://github.com/launchbadge/sqlx/issues/1260
rm -f indexer.db
cargo sqlx database create --database-url sqlite:indexer.db
cargo sqlx migrate run --database-url sqlite:indexer.db
cargo sqlx prepare --database-url sqlite:indexer.db