# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.245.2/containers/rust/.devcontainer/base.Dockerfile

# [Choice] Debian OS version (use 0-bullseye on local arm64/Apple Silicon): 0-buster, 0-bullseye bookworm
ARG VARIANT
FROM mcr.microsoft.com/vscode/devcontainers/rust:${VARIANT}

# Install additional packages
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends protobuf-compiler libprotobuf-dev libclang-dev

RUN apt install -y libgmp3-dev

RUN rustup toolchain install 1.70.0 && \
	rustup default 1.70.0 && rustup component add clippy && rustup component add rustfmt

# Install Python
ARG PYTHON_PATH=/usr/local/python
ENV PIPX_HOME=/usr/local/py-utils \
    PIPX_BIN_DIR=/usr/local/py-utils/bin
ENV PATH=${PYTHON_PATH}/bin:${PATH}:${PIPX_BIN_DIR}
COPY .devcontainer/library-scripts/python-debian.sh /tmp/library-scripts/
RUN apt-get update && bash /tmp/library-scripts/python-debian.sh "3.9.6" "${PYTHON_PATH}" "${PIPX_HOME}"

# Install nodejs
ENV NVM_DIR="/usr/local/share/nvm"
ENV NVM_SYMLINK_CURRENT=true \
    PATH=${NVM_DIR}/current/bin:${PATH}
COPY .devcontainer/library-scripts/node-debian.sh /tmp/library-scripts/
RUN apt-get update && bash /tmp/library-scripts/node-debian.sh "${NVM_DIR}"

# Install dojoup and scarb for vscode user
USER vscode
RUN curl -L https://install.dojoengine.org | bash
RUN curl --proto '=https' --tlsv1.2 -sSf https://docs.swmansion.com/scarb/install.sh | bash

ENV PATH=${PATH}:/workspaces/dojo/target/release
