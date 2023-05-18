#!/bin/bash
set -e

pushd $(dirname "$0")/..

if ! command -v sqlite3 &> /dev/null
then 
    echo "sqlite3 could not be found"
    exit
fi

if [ ! -f indexer.db ]; then
    echo "indexer.db file not found, run reset_db.sh"
    exit
fi

sqlite3 indexer.db < src/tests/fixtures/seed.sql
echo "Database seeded with mock data"