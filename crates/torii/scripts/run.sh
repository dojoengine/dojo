#!/bin/bash
pushd $(dirname "$0")/..

cargo run --bin torii -- --database-url indexer.db