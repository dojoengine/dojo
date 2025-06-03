# Dockerfile for minimum system dependencies and Dojoup installation local test.

FROM ubuntu:24.04

# System dependencies for dojoup installation.
RUN apt-get update && \
    apt-get install -y curl unzip git jq ca-certificates && \
    apt-get clean

# Create a non-root user.
RUN useradd -ms /bin/bash dojo
USER dojo
WORKDIR /home/dojo

COPY --chown=dojo:dojo install install
COPY --chown=dojo:dojo dojoup dojoup

RUN ./install ./dojoup && \
    . "${XDG_CONFIG_HOME:-$HOME}"/.dojo/env && \
    dojoup install

# Copy and run the post-install check
COPY --chown=dojo:dojo post_install_check post_install_check

CMD ["./post_install_check"]
