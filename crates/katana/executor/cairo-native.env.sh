#!/bin/sh

# Taken from `lambdaclass/cairo_native`.
#
# It sets the LLVM environment variables.
#
# You can copy this file to .envrc/.env and adapt it for your environment.

case $(uname) in
  Darwin)
    # If installed with brew
    LIBRARY_PATH=/opt/homebrew/lib
    MLIR_SYS_190_PREFIX="$(brew --prefix llvm@19)"
    LLVM_SYS_191_PREFIX="$(brew --prefix llvm@19)"
    TABLEGEN_190_PREFIX="$(brew --prefix llvm@19)"

    export LIBRARY_PATH
    export MLIR_SYS_190_PREFIX
    export LLVM_SYS_191_PREFIX
    export TABLEGEN_190_PREFIX
  ;;
  Linux)
    # If installed from Debian/Ubuntu repository:
    MLIR_SYS_190_PREFIX=/usr/lib/llvm-19
    LLVM_SYS_191_PREFIX=/usr/lib/llvm-19
    TABLEGEN_190_PREFIX=/usr/lib/llvm-19

    export MLIR_SYS_190_PREFIX
    export LLVM_SYS_191_PREFIX
    export TABLEGEN_190_PREFIX
  ;;
esac

# Blockifier build script
CAIRO_NATIVE_RUNTIME_LIBRARY="$(pwd)/target/release/libcairo_native_runtime.a"
export CAIRO_NATIVE_RUNTIME_LIBRARY
