FROM debian:buster-slim as base

LABEL description="Dojo is a provable game engine and toolchain for building onchain games and autonomous worlds with Cairo" \
    authors="tarrence <tarrence@cartridge.gg>" \
    source="https://github.com/dojoengine/dojo" \
    documentation="https://book.dojoengine.org/"

FROM base as amd64

RUN ls -R

COPY artifacts/x86_64-unknown-linux-gnu/release/katana /usr/local/bin/katana
COPY artifacts/x86_64-unknown-linux-gnu/release/sozo /usr/local/bin/sozo
COPY artifacts/x86_64-unknown-linux-gnu/release/torii /usr/local/bin/torii

FROM base as arm64

RUN ls -R

COPY artifacts/aarch64-unknown-linux-gnu/release/katana /usr/local/bin/katana
COPY artifacts/aarch64-unknown-linux-gnu/release/sozo /usr/local/bin/sozo
COPY artifacts/aarch64-unknown-linux-gnu/release/torii /usr/local/bin/torii
