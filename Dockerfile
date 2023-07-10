FROM rust:slim-buster as builder
RUN apt-get -y update; \
    apt-get install -y --no-install-recommends \
    libssl-dev make clang-11 g++ llvm protobuf-compiler \
    pkg-config libz-dev zstd git; \
    apt-get autoremove -y; \
    apt-get clean; \
    rm -rf /var/lib/apt/lists/*

WORKDIR /dojo
COPY . .
RUN cargo build --release --config net.git-fetch-with-cli=true

FROM debian:buster-slim
LABEL description="Dojo is a provable game engine and toolchain for building onchain games and autonomous worlds with Cairo" \
    authors="tarrence <tarrence@cartridge.gg>" \
    source="https://github.com/dojoengine/dojo" \
    documentation="https://book.dojoengine.org/"

COPY --from=builder /dojo/target/release/katana /katana
COPY --from=builder /dojo/target/release/sozo /sozo
COPY --from=builder /dojo/target/release/torii /torii
