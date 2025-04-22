FROM ubuntu:24.04 AS builder

RUN apt-get update && apt install -y git libtool automake autoconf make

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

FROM ubuntu:24.04 AS base

RUN apt-get update && \
    apt-get install -y curl ca-certificates tini && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/* && \
    cp /usr/bin/tini /tini

ENTRYPOINT ["/tini", "--"]

ARG TARGETPLATFORM

LABEL description="Dojo is a provable game engine and toolchain for building onchain games and autonomous worlds with Cairo" \
    authors="tarrence <tarrence@cartridge.gg>" \
    source="https://github.com/dojoengine/dojo" \
    documentation="https://book.dojoengine.org/"

COPY --from=artifacts --chmod=755 $TARGETPLATFORM/sozo /usr/local/bin/

# We may not want to install sozo via dojoup though, only Katana and Torii?
# Or we can let the use decide.
COPY dojoup/dojoup /usr/local/bin/dojoup
# RUN dojoup -v $DOJO_VERSION

COPY --from=builder /usr/local/bin/curtail /usr/local/bin/curtail
