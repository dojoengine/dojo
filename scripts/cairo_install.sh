#!/bin/bash

set -euxo pipefail

if [ -d ./workspace ]; then
    git -C ./workspace pull
else
    mkdir -p ./workspace
	git clone --depth 1 https://github.com/starkware-libs/cairo.git ./workspace
fi

cargo build