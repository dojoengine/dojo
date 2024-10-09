FROM debian:bookworm-slim as builder

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

FROM debian:bookworm-slim as base

ENV TINI_VERSION v0.19.0
ADD https://github.com/krallin/tini/releases/download/${TINI_VERSION}/tini /tini
RUN chmod +x /tini
ENTRYPOINT ["/tini", "--"]

ARG TARGETPLATFORM

LABEL description="Dojo is a provable game engine and toolchain for building onchain games and autonomous worlds with Cairo" \
    authors="tarrence <tarrence@cartridge.gg>" \
    source="https://github.com/dojoengine/dojo" \
    documentation="https://book.dojoengine.org/"

COPY --from=artifacts $TARGETPLATFORM/katana /usr/local/bin/katana
COPY --from=artifacts $TARGETPLATFORM/sozo /usr/local/bin/sozo
COPY --from=artifacts $TARGETPLATFORM/torii /usr/local/bin/torii

COPY --from=builder /usr/local/bin/curtail /usr/local/bin/curtail

RUN chmod +x /usr/local/bin/katana \
    && chmod +x /usr/local/bin/sozo \
    && chmod +x /usr/local/bin/torii
