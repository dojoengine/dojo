ARG DOJO_VERSION=v1.5.0-preview.0
FROM ghcr.io/dojoengine/dojo:${DOJO_VERSION}
ARG DOJO_VERSION
ARG RUST_VERSION

# Base image does not seem to expose rustup in PATH for subsequent RUN commands.
# Explicitly install Rust using rustup.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_VERSION}
ENV PATH="/root/.cargo/bin:${PATH}"

# Set non-interactive frontend for apt-get
ENV DEBIAN_FRONTEND=noninteractive
ENV _RJEM_MALLOC_CONF="abort_conf:true,junk:false"

# Install additional packages potentially not in the base image
RUN apt-get update && apt-get install -y --no-install-recommends \
    protobuf-compiler \
    libprotobuf-dev \
    libclang-dev \
    libgmp3-dev \
    libssl-dev \
    libzstd-dev \
    gcc \
    git \
    gh \
    make \
    pkg-config \
    software-properties-common \
    unzip \
    && rm -rf /var/lib/apt/lists/*

ENV OPENSSL_DIR=/usr

RUN curl -fsSL https://bun.sh/install | bash && . /root/.bashrc

RUN curl -L https://foundry.paradigm.xyz/ | bash && . /root/.bashrc && foundryup
ENV PATH="${PATH}:/root/.foundry/bin"

RUN rustup component add clippy rustfmt
RUN rustup component add llvm-tools-preview

RUN rustup target add x86_64-pc-windows-msvc && \
    rustup target add wasm32-unknown-unknown

RUN cargo install cargo-llvm-cov

# Platform specific tooling (hurl, llvm-tools)
ARG TARGETPLATFORM

# Note: Assuming rustup is available in the base image for adding components
RUN if [ "$TARGETPLATFORM" = "linux/arm64" ] ; then \
    curl -L https://github.com/Orange-OpenSource/hurl/releases/download/4.1.0/hurl-4.1.0-aarch64-unknown-linux-gnu.tar.gz -o hurl.tar.gz && \
    tar -xzf hurl.tar.gz && \
    mv hurl-*-aarch64-unknown-linux-gnu/hurl /usr/local/bin/ && \
    rm -r hurl-*-aarch64-unknown-linux-gnu && \
    rm hurl.tar.gz && \
    rustup component add llvm-tools-preview; \
    elif [ "$TARGETPLATFORM" = "linux/amd64" ] ; then \
    curl -L https://github.com/Orange-OpenSource/hurl/releases/download/4.1.0/hurl-4.1.0-x86_64-unknown-linux-gnu.tar.gz -o hurl.tar.gz && \
    tar -xzf hurl.tar.gz && \
    mv hurl-*-x86_64-unknown-linux-gnu/hurl /usr/local/bin/ && \
    rm -r hurl-*-x86_64-unknown-linux-gnu && \
    rm hurl.tar.gz && \
    rustup component add llvm-tools-preview && \
    rustup target add x86_64-fortanix-unknown-sgx; \
    fi
