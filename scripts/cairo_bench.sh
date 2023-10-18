#!/bin/bash

if [ "$#" -lt 1 ]; then
  echo "Usage: $0 [set|diff]"
  exit 1
fi

if [ $1 = "set" ]; then
    source scripts/cairo_test.sh -f bench_ | grep "gas usage est.:" | sort | tee benches.txt
elif [ $1 = "diff" ]; then
    source scripts/cairo_test.sh -f bench_ | grep "gas usage est.:" | sort | diff benches.txt -
fi
