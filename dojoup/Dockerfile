# Dockerfile for minimum system dependencies and Dojoup installation local test.

FROM ubuntu:24.04

# System dependencies for dojo/katana/torii
RUN apt-get update && \
    apt-get install -y curl unzip git jq ca-certificates && \
    apt-get clean

# Create a non-root user (optional but recommended)
RUN useradd -ms /bin/bash dojo
USER dojo
WORKDIR /home/dojo

# Copy the local dojoup folder into the container
COPY --chown=dojo:dojo dojoup ./dojoup

# Install Dojoup using the install script
RUN ./dojoup/install ./dojoup/dojoup && \
    . "$HOME/.dojo/env" && \
    dojoup install

# Copy and run the post-install check
COPY --chown=dojo:dojo dojoup/post_install_check.sh ./post_install_check.sh
RUN chmod +x ./post_install_check.sh

CMD ["./post_install_check.sh"]
