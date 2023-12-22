# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.245.2/containers/rust/.devcontainer/base.Dockerfile

# [Choice] Debian OS version (use bookworm on local arm64/Apple Silicon): buster, bullseye, bookworm

ARG VARIANT
FROM mcr.microsoft.com/vscode/devcontainers/rust:${VARIANT}

# Install additional packages
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends protobuf-compiler libprotobuf-dev libclang-dev

RUN apt install -y gh libgmp3-dev

COPY rust-toolchain.toml .

# Install cargo-binstall
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

RUN rustup toolchain install $(cat rust-toolchain.toml | grep channel | cut -d\" -f2) && \
    rustup default $(cat rust-toolchain.toml | grep channel | cut -d\" -f2) && \
    rustup component add clippy && \
    rustup component add rustfmt

RUN rustup toolchain install nightly && \
    rustup component add rustfmt --toolchain nightly

RUN rustup target add x86_64-pc-windows-msvc && \
    rustup target add wasm32-unknown-unknown

RUN cargo binstall cargo-get cargo-nextest cargo-llvm-cov cargo-release --secure -y

# Platform specific tooling
ARG TARGETPLATFORM
RUN if [ "$TARGETPLATFORM" = "linux/arm64" ] ; then \
    curl -L https://github.com/Orange-OpenSource/hurl/releases/download/4.1.0/hurl-4.1.0-aarch64-unknown-linux-gnu.tar.gz -o hurl.tar.gz && \
    tar -xzf hurl.tar.gz && \
    mv hurl-4.1.0-aarch64-unknown-linux-gnu/hurl /usr/local/bin/ && \
    rm -r hurl-4.1.0-aarch64-unknown-linux-gnu && \
    rm hurl.tar.gz && \
    rustup component add llvm-tools-preview --toolchain 1.70.0-aarch64-unknown-linux-gnu; \
    elif [ "$TARGETPLATFORM" = "linux/amd64" ] ; then \
    curl -L https://github.com/Orange-OpenSource/hurl/releases/download/4.1.0/hurl-4.1.0-x86_64-unknown-linux-gnu.tar.gz -o hurl.tar.gz && \
    tar -xzf hurl.tar.gz && \
    mv hurl-4.1.0-x86_64-unknown-linux-gnu/hurl /usr/local/bin/ && \
    rm -r hurl-4.1.0-x86_64-unknown-linux-gnu && \
    rm hurl.tar.gz && \
    rustup component add llvm-tools-preview --toolchain 1.70.0-x86_64-unknown-linux-gnu && \
    rustup target add x86_64-fortanix-unknown-sgx --toolchain nightly; \
    fi

RUN curl -L https://install.dojoengine.org | bash
RUN curl --proto '=https' --tlsv1.2 -sSf https://docs.swmansion.com/scarb/install.sh | bash
ENV PATH=${PATH}:/root/.dojo/bin
RUN dojoup

RUN chown -R root:root /usr/local/cargo
RUN chmod -R 700 /usr/local/cargo
