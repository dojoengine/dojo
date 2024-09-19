FROM rust:1.80.0-slim as chef

# Install libclang and other necessary tools
RUN apt-get update && \
    apt-get install -y clang llvm-dev libclang-dev git libtool automake autoconf make curl

RUN apt-get install -y protobuf-compiler

# Verify and set LIBCLANG_PATH environment variable
RUN find /usr -name "libclang.so*" && \
    export LIBCLANG_PATH=$(find /usr -name "libclang.so*" | head -n 1 | xargs dirname)

RUN rustup install 1.79.0
RUN rustup component add cargo clippy rust-docs rust-std rustc rustfmt

RUN cargo install cargo-chef

FROM chef AS planner

WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
RUN cargo build --release --bins

# Build application
COPY . .
ENV PATH="/root/.cargo/bin:${PATH}"

RUN cargo build --release --bins

FROM rust:1-alpine

WORKDIR /

COPY --from=builder /app/target/release/katana /app/artifacts/
COPY --from=builder /app/target/release/torii /app/artifacts/
COPY --from=builder /app/target/release/sozo /app/artifacts/
COPY --from=builder /app/target/release/saya /app/artifacts/
