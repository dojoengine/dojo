#!/bin/bash

if [ "$#" -lt 1 ]; then
  echo "Usage: $0 [set|diff|assert]"
  exit 1
fi

function run()
{
    source scripts/cairo_test.sh -f bench_ | grep "DEBUG" | awk 'match($0, /0x[0-9a-fA-F]+/) {
        hex = substr($0, RSTART, RLENGTH);
        for (i = - 29; i < 1; i += 2) {
            printf "%c", strtonum("0x" substr(hex, length(hex) + i, 2));
        }
        print ": " strtonum(substr(hex, 1, length(hex) - 30));
    }'
}

if [ $1 = "set" ]; then
    run | sort | tee benches.txt
elif [ $1 = "diff" ]; then
    run | sort | diff benches.txt -
elif [ $1 = "assert" ]; then
    run | sort | cmp benches.txt -
else
    echo "Usage: $0 [set|diff|assert]"
    exit 1
fi
