FROM debian:bookworm-slim as base

ARG TARGETPLATFORM

LABEL description="Dojo is a provable game engine and toolchain for building onchain games and autonomous worlds with Cairo" \
    authors="tarrence <tarrence@cartridge.gg>" \
    source="https://github.com/dojoengine/dojo" \
    documentation="https://book.dojoengine.org/"

COPY --from=artifacts $TARGETPLATFORM/katana /usr/local/bin/katana
COPY --from=artifacts $TARGETPLATFORM/sozo /usr/local/bin/sozo
COPY --from=artifacts $TARGETPLATFORM/torii /usr/local/bin/torii

RUN chmod +x /usr/local/bin/katana \
    && chmod +x /usr/local/bin/sozo \
    && chmod +x /usr/local/bin/torii