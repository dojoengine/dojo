#!/bin/bash

# Define the image name and tag
IMAGE_NAME="dojoengine"
IMAGE_TAG="latest"
DOCKERHUB_USERNAME="your_dockerhub_username"

# Build the Docker image
docker build -t ${IMAGE_NAME}:${IMAGE_TAG} .

# Log in to Docker Hub
docker login --username ${DOCKERHUB_USERNAME}

# Push the Docker image to Docker Hub
docker push ${IMAGE_NAME}:${IMAGE_TAG}