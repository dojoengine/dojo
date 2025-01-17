# See here for image contents: https://hub.docker.com/layers/library/rust/1.81.0-slim/images/sha256-ba4ee661bb466ab49f6ceb8c6d9e9f9784bba7c6e45225187cd3c7fb1fbc12ce

ARG RUST_VERSION=1.81.0
FROM rust:${RUST_VERSION}-slim
ARG RUST_VERSION

# Install additional packages
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends protobuf-compiler libprotobuf-dev libclang-dev libzstd-dev make

RUN apt-get install -y gh libgmp3-dev software-properties-common curl git

RUN curl -L https://foundry.paradigm.xyz/ | bash && . /root/.bashrc && foundryup
ENV PATH="${PATH}:/root/.foundry/bin"

# To allow independent workflow of the container, the rust-toolchain is explicitely given.
RUN echo ${RUST_VERSION} > rust_toolchain_version
# Make sure to sync the nightly version with the scripts in ./scripts
RUN echo "nightly-2024-08-28" > nightly_rust_toolchain_version

# Install cargo-binstall
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

RUN rustup toolchain install $(cat rust_toolchain_version) && \
    rustup default $(cat rust_toolchain_version) && \
    rustup component add clippy && \
    rustup component add rustfmt

RUN rustup toolchain install $(cat nightly_rust_toolchain_version) && \
	rustup component add rustfmt clippy --toolchain $(cat nightly_rust_toolchain_version)

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
    rustup component add llvm-tools-preview --toolchain $(cat rust_toolchain_version)-aarch64-unknown-linux-gnu; \
    elif [ "$TARGETPLATFORM" = "linux/amd64" ] ; then \
    curl -L https://github.com/Orange-OpenSource/hurl/releases/download/4.1.0/hurl-4.1.0-x86_64-unknown-linux-gnu.tar.gz -o hurl.tar.gz && \
    tar -xzf hurl.tar.gz && \
    mv hurl-4.1.0-x86_64-unknown-linux-gnu/hurl /usr/local/bin/ && \
    rm -r hurl-4.1.0-x86_64-unknown-linux-gnu && \
    rm hurl.tar.gz && \
    rustup component add llvm-tools-preview --toolchain $(cat rust_toolchain_version)-x86_64-unknown-linux-gnu && \
    rustup target add x86_64-fortanix-unknown-sgx --toolchain $(cat nightly_rust_toolchain_version); \
    fi

ARG DOJO_VERSION=stable
ARG BUILD_TYPE=default
RUN curl -L https://install.dojoengine.org | bash
RUN curl --proto '=https' --tlsv1.2 -sSf https://docs.swmansion.com/scarb/install.sh | bash
ENV PATH=${PATH}:/root/.dojo/bin
RUN if [ "$BUILD_TYPE" = "release" ]; then \
    echo "Installing Dojo version $DOJO_VERSION"; \
    dojoup -v $DOJO_VERSION; \
    elif [ "$BUILD_TYPE" = "push" ]; then \
    echo "Installing Dojo at commit $DOJO_VERSION"; \
    dojoup -c $DOJO_VERSION; \
    else \
    echo "Installing latest stable version of Dojo"; \
    dojoup; \
    fi

RUN chown -R root:root /usr/local/cargo
RUN chmod -R 700 /usr/local/cargo
