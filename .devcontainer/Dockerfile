# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.245.2/containers/rust/.devcontainer/base.Dockerfile

# [Choice] Debian OS version (use bookworm on local arm64/Apple Silicon): buster, bullseye, bookworm
ARG VARIANT
ARG TARGETPLATFORM

FROM mcr.microsoft.com/vscode/devcontainers/rust:${VARIANT}

# Install additional packages
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends protobuf-compiler libprotobuf-dev libclang-dev

RUN apt install -y libgmp3-dev

COPY rust-toolchain.toml .

# Install cargo-binstall
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

RUN rustup toolchain install $(cat rust-toolchain.toml | grep channel | cut -d\" -f2) && \
    rustup default $(cat rust-toolchain.toml | grep channel | cut -d\" -f2) && \
    rustup component add clippy && \
    rustup component add rustfmt

RUN rustup target add x86_64-pc-windows-msvc && \
    rustup target add wasm32-unknown-unknown

RUN rustup toolchain install nightly && \
    rustup target add x86_64-fortanix-unknown-sgx --toolchain nightly

RUN cargo binstall cargo-nextest cargo-llvm-cov --secure -y
RUN if [ "$TARGETPLATFORM" = "linux/arm64" ] ; then \
    rustup component add llvm-tools-preview --toolchain 1.70.0-aarch64-unknown-linux-gnu; \
    elif [ "$TARGETPLATFORM" = "linux/amd64" ] ; then \
    rustup component add llvm-tools-preview --toolchain 1.70.0-x86_64-unknown-linux-gnu; \
    fi

RUN curl -L https://install.dojoengine.org | bash
RUN curl --proto '=https' --tlsv1.2 -sSf https://docs.swmansion.com/scarb/install.sh | bash
ENV PATH=${PATH}:/root/.dojo/bin
RUN dojoup

RUN chown -R root:root /usr/local/cargo
RUN chmod -R 700 /usr/local/cargo