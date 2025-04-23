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
    apt-get install -y curl ca-certificates tini jq && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/* && \
    cp /usr/bin/tini /tini

ENTRYPOINT ["/tini", "--"]

ARG DOJO_VERSION

LABEL description="Dojo is a provable game engine and toolchain for building onchain games and autonomous worlds with Cairo" \
    authors="tarrence <tarrence@cartridge.gg>" \
    source="https://github.com/dojoengine/dojo" \
    documentation="https://book.dojoengine.org/"

RUN curl -L https://install.dojoengine.org | bash
RUN . ~/.dojo/env && dojoup install $DOJO_VERSION

COPY --from=builder /usr/local/bin/curtail /usr/local/bin/curtail
