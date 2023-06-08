#!/bin/bash
pushd $(dirname "$0")/..

cargo test --bin torii -- --nocapture