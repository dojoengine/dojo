FROM debian:bookworm-slim

ARG SOZO_VERSION
ARG TARGETARCH

RUN apt-get update \
  && apt-get install -y --no-install-recommends git curl wget ca-certificates \
  && rm -rf /var/lib/apt/lists/*

# Archive is provided by the release workflow (linux/amd64 and linux/arm64 variants).
COPY artifacts/sozo_${SOZO_VERSION}_linux_${TARGETARCH}.tar.gz /tmp/sozo.tar.gz

RUN tar -xzf /tmp/sozo.tar.gz -C /usr/local/bin sozo \
  && rm /tmp/sozo.tar.gz \
  && chmod +x /usr/local/bin/sozo

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

RUN asdf plugin add scarb https://github.com/software-mansion/asdf-scarb.git && \
asdf plugin add starknet-foundry https://github.com/foundry-rs/asdf-starknet-foundry && \
asdf install && \
asdf reshim

ENTRYPOINT ["sozo"]
