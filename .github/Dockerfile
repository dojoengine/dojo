FROM rust:slim as chef
ARG CAIRO_NATIVE=false

# Install libclang and other necessary tools
RUN apt-get update && \
	apt-get install -y clang llvm-dev libclang-dev git libtool automake autoconf make curl protobuf-compiler && \
	rm -rf /var/lib/apt/lists/*

# Verify and set LIBCLANG_PATH environment variable
RUN find /usr -name "libclang.so*" && \
	export LIBCLANG_PATH=$(find /usr -name "libclang.so*" | head -n 1 | xargs dirname)

COPY rust-toolchain.toml .
RUN rustup install $(cat rust-toolchain.toml | grep channel | cut -d' ' -f3 | tr -d '"')
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
RUN cargo build --release --bins # pre-cache some stuff

# Build application
COPY . .
ENV PATH="/root/.cargo/bin:${PATH}"

RUN if [ "$CAIRO_NATIVE" = "true" ]; then \
	# Install cairo-native dependencies
	sudo apt-get install llvm-19 llvm-19-dev llvm-19-runtime clang-19 clang-tools-19 lld-19 libpolly-19-dev libmlir-19-dev mlir-19-tools; \
	cargo build --release --bins --features cairo-native; \
	else \
	cargo build --release --bins; \
	fi

FROM rust:1-alpine

WORKDIR /

COPY --from=builder /app/target/release/katana /app/artifacts/
COPY --from=builder /app/target/release/torii /app/artifacts/
COPY --from=builder /app/target/release/sozo /app/artifacts/
