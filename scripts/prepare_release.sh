#!/bin/bash
set -euxo pipefail

prev_version=$(cargo get workspace.package.version)
next_version=$1

find . -type f -name "*.toml" -exec sed -i "" "s/version = \"$prev_version\"/version = \"$next_version\"/g" {} \;
find . -type f -name "*.toml" -exec sed -i "" "s/dojo_plugin = \"$prev_version\"/dojo_plugin = \"$next_version\"/g" {} \;

scripts/clippy.sh

git commit -am "Prepare v$1"
git tag -a "v$1" -m "Version $1"
# git push origin --tags