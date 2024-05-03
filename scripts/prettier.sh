#/bin/bash

# Formats all the markdown and yaml files in the repository.

prettier --check "**/*.md"
prettier --check "**/*.{yaml,yml}"
