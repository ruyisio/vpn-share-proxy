#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
OUTPUT_DIR="${ROOT_DIR}/dist"
CACHE_VOL="vpn-share-proxy-app-gradle-cache"
IMAGE_NAME="vpn-share-proxy-app-builder:latest"

echo "=========================================="
echo " Building vpn-share-proxy-app APK via Docker"
echo " (persistent Gradle cache → incremental)"
echo "=========================================="

mkdir -p "${OUTPUT_DIR}"
docker volume create "${CACHE_VOL}" >/dev/null

echo "-> Ensuring builder image (deps only, no project compile)..."
docker build -t "${IMAGE_NAME}" -f "${SCRIPT_DIR}/Dockerfile" "${SCRIPT_DIR}"

# Parallelism: use host cores; keep daemon so warm rebuilds are fast
CPUS="$(nproc 2>/dev/null || echo 8)"

echo "-> Compiling with cached Gradle home (volume: ${CACHE_VOL})..."
docker run --rm \
  --cpus="${CPUS}" \
  -e "GRADLE_OPTS=-Dorg.gradle.daemon=true -Dorg.gradle.parallel=true -Dorg.gradle.caching=true -Dorg.gradle.workers.max=${CPUS} -Xmx8g" \
  -v "${ROOT_DIR}:/workspace" \
  -v "${CACHE_VOL}:/gradle-cache" \
  -v "${OUTPUT_DIR}:/output" \
  -w /workspace \
  "${IMAGE_NAME}"

echo "-> Build complete! Outputs in ${OUTPUT_DIR}:"
ls -lh "${OUTPUT_DIR}" || true
echo "=========================================="
