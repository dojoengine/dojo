FROM debian:bookworm-slim as builder

RUN apt-get update && apt install -y git libtool automake autoconf make tini curl

## install dojoengine
RUN curl -L https://install.dojoengine.org | bash

SHELL ["/bin/bash", "-c"]

RUN source /root/.bashrc && dojoup

FROM debian:bookworm-slim as base

COPY --from=builder /usr/bin/tini /tini
COPY --from=builder /root/.dojo/bin/katana /usr/local/bin/katana
COPY --from=builder /root/.dojo/bin/sozo /usr/local/bin/sozo
COPY --from=builder /root/.dojo/bin/torii /usr/local/bin/torii

LABEL description="Dojo is a provable game engine and toolchain for building onchain games and autonomous worlds with Cairo" \
    authors="tarrence <tarrence@cartridge.gg>" \
    source="https://github.com/dojoengine/dojo" \
    documentation="https://book.dojoengine.org/"
