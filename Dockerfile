FROM debian:bookworm-slim as builder

RUN apt-get update && apt install -y git libtool automake autoconf make tini

RUN git clone https://github.com/Comcast/Infinite-File-Curtailer.git curtailer \
    && cd curtailer \
    && libtoolize \
    && aclocal \
    && autoheader \
    && autoconf \
    && automake --add-missing \
    && ./configure \
    && make \
    && make install \
    && curtail --version

FROM debian:bookworm-slim as base

COPY --from=builder /usr/bin/tini /tini
ENTRYPOINT ["/tini", "--"]

ARG TARGETPLATFORM

LABEL description="Dojo is a provable game engine and toolchain for building onchain games and autonomous worlds with Cairo" \
    authors="tarrence <tarrence@cartridge.gg>" \
    source="https://github.com/dojoengine/dojo" \
    documentation="https://book.dojoengine.org/"

COPY --from=artifacts --chmod=755 $TARGETPLATFORM/katana $TARGETPLATFORM/sozo $TARGETPLATFORM/torii /usr/local/bin/

COPY --from=builder /usr/local/bin/curtail /usr/local/bin/curtail
