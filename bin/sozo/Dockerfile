FROM debian:bookworm-slim

ARG SOZO_VERSION
ARG TARGETARCH

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*

# Archive is provided by the release workflow (linux/amd64 and linux/arm64 variants).
COPY artifacts/sozo_${SOZO_VERSION}_linux_${TARGETARCH}.tar.gz /tmp/sozo.tar.gz

RUN tar -xzf /tmp/sozo.tar.gz -C /usr/local/bin sozo \
  && rm /tmp/sozo.tar.gz \
  && chmod +x /usr/local/bin/sozo

ENTRYPOINT ["sozo"]
