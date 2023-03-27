# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.245.2/containers/rust/.devcontainer/base.Dockerfile

# [Choice] Debian OS version (use bullseye on local arm64/Apple Silicon): buster, bullseye
ARG VARIANT
FROM mcr.microsoft.com/vscode/devcontainers/rust:0-${VARIANT}

# Install additional packages
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends protobuf-compiler libprotobuf-dev

RUN rustup toolchain install nightly-2022-11-03 \
    && rustup default nightly-2022-11-03 && rustup component add clippy
