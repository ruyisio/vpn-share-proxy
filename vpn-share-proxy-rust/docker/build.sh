#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
OUTPUT_DIR="${ROOT_DIR}/target/dist"

echo "=========================================="
echo " Building vpn-share-proxy (aarch64-unknown-linux-musl)"
echo "=========================================="

mkdir -p "${OUTPUT_DIR}"

IMAGE_NAME="vpn-share-proxy-builder:latest"

echo "-> Building Docker builder image..."
docker build -t "${IMAGE_NAME}" -f "${SCRIPT_DIR}/Dockerfile" "${ROOT_DIR}"

echo "-> Extracting compiled binaries to ${OUTPUT_DIR}..."
docker run --rm -v "${OUTPUT_DIR}:/output" "${IMAGE_NAME}"

echo "-> Build succeeded! Binaries available in:"
ls -lh "${OUTPUT_DIR}"
echo "=========================================="
