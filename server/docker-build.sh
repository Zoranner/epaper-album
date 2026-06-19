#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
IMAGE_NAME="epaper-album-server"
IMAGE_TAG="${1:-latest}"

echo "[docker-build] Building image: ${IMAGE_NAME}:${IMAGE_TAG}"

if docker buildx version &>/dev/null; then
    docker buildx build \
        -t "${IMAGE_NAME}:${IMAGE_TAG}" \
        -f "${SCRIPT_DIR}/Dockerfile" \
        --load \
        "${REPO_ROOT}"
else
    DOCKER_BUILDKIT=1 docker build \
        -t "${IMAGE_NAME}:${IMAGE_TAG}" \
        -f "${SCRIPT_DIR}/Dockerfile" \
        "${REPO_ROOT}"
fi

echo "[docker-build] Done: ${IMAGE_NAME}:${IMAGE_TAG}"
