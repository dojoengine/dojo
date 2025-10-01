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
    apt-get install -y curl wget ca-certificates libssl-dev tini jq git && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/* && \
    cp /usr/bin/tini /tini

ENTRYPOINT ["/tini", "--"]

ARG DOJO_VERSION
ARG TARGETARCH

LABEL description="Dojo is a provable game engine and toolchain for building onchain games and autonomous worlds with Cairo" \
    authors="tarrence <tarrence@cartridge.gg>" \
    source="https://github.com/dojoengine/dojo" \
    documentation="https://book.dojoengine.org/"

# Define the ASDF related variables.
# Note that the data dir is also invovled.
ENV ASDF_VERSION=v0.18.0
ENV ASDF_DIR=/opt/asdf
ENV ASDF_BIN_DIR=${ASDF_DIR}/bin
ENV ASDF_DATA_DIR=${ASDF_DIR}/

# Add to the PATH + ensure the ASDF_DATA_DIR is also set,
# to have next invocation of asdf working as expected.
ENV PATH="${ASDF_BIN_DIR}:${ASDF_DATA_DIR}/shims:$PATH"
ENV ASDF_DATA_DIR=${ASDF_DATA_DIR}

# Install ASDF from pre-built binaries github release (easiest way IMHO).
RUN wget -q https://github.com/asdf-vm/asdf/releases/download/${ASDF_VERSION}/asdf-${ASDF_VERSION}-linux-${TARGETARCH}.tar.gz -O /tmp/asdf.tar.gz && \
    mkdir -p $ASDF_BIN_DIR && \
    tar -xzf /tmp/asdf.tar.gz -C $ASDF_BIN_DIR && \
    rm /tmp/asdf.tar.gz && \
    ls -laR ${ASDF_DIR}

RUN chmod +x $ASDF_BIN_DIR/asdf && ls -alR $ASDF_DIR/
RUN asdf --version

COPY .tool-versions /root/.tool-versions
# Add sozo and torii to the tool-versions file for all in one.
RUN echo "sozo ${DOJO_VERSION}" >> /root/.tool-versions
RUN echo "torii ${DOJO_VERSION}" >> /root/.tool-versions

RUN asdf plugin add scarb https://github.com/software-mansion/asdf-scarb.git && \
    asdf plugin add starknet-foundry https://github.com/foundry-rs/asdf-starknet-foundry && \
    asdf plugin add katana https://github.com/dojoengine/asdf-katana.git && \
    asdf plugin add sozo https://github.com/dojoengine/asdf-sozo.git && \
    asdf plugin add torii https://github.com/dojoengine/asdf-torii.git && \
    asdf install && \
    asdf reshim

COPY --from=builder /usr/local/bin/curtail /usr/local/bin/curtail
