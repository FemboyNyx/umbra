#!/usr/bin/env bash
# Build a relocatable Linux tar.gz from a Flutter Linux release bundle.
#
# Usage:
#   packaging/linux/build-tarball.sh <flutter-bundle-dir> [output-tar.gz]
#
# <flutter-bundle-dir> is typically:
#   app/build/linux/x64/release/bundle
#
# Environment (optional):
#   UMBRA_VERSION   Version string embedded in the archive name (default: 0.0.0-dev)
#   DEST_DIR        Directory to write the archive into (default: cwd)
#
# Exit codes: 0 success, 1 usage/input error, 2 pack failure.
set -euo pipefail

usage() {
  echo "Usage: $0 <flutter-bundle-dir> [output-tar.gz]" >&2
  exit 1
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
fi

BUNDLE_DIR="${1:-}"
if [[ -z "${BUNDLE_DIR}" ]]; then
  usage
fi
if [[ ! -d "${BUNDLE_DIR}" ]]; then
  echo "error: flutter bundle directory not found: ${BUNDLE_DIR}" >&2
  exit 1
fi
if [[ ! -x "${BUNDLE_DIR}/umbra" && ! -f "${BUNDLE_DIR}/umbra" ]]; then
  # Flutter may name the binary after the project; accept any executable at top level.
  if ! find "${BUNDLE_DIR}" -maxdepth 1 -type f -executable | grep -q .; then
    echo "error: no executable found in bundle: ${BUNDLE_DIR}" >&2
    exit 1
  fi
fi

VERSION="${UMBRA_VERSION:-0.0.0-dev}"
DEST_DIR="${DEST_DIR:-.}"
mkdir -p "${DEST_DIR}"

DEFAULT_OUT="${DEST_DIR}/umbra-${VERSION}-linux-x64.tar.gz"
OUT="${2:-${DEFAULT_OUT}}"

STAGE="$(mktemp -d)"
cleanup() { rm -rf "${STAGE}"; }
trap cleanup EXIT

APP_ROOT="${STAGE}/umbra"
mkdir -p "${APP_ROOT}"
cp -a "${BUNDLE_DIR}/." "${APP_ROOT}/"

# Ship desktop + metainfo next to the binary for packagers / AppImage reuse.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
install -m 0644 "${SCRIPT_DIR}/umbra.desktop" "${APP_ROOT}/umbra.desktop"
install -m 0644 "${SCRIPT_DIR}/io.github.femboynyx.umbra.metainfo.xml" \
  "${APP_ROOT}/io.github.femboynyx.umbra.metainfo.xml"
if [[ -f "${SCRIPT_DIR}/../icons/umbra-256.png" ]]; then
  install -m 0644 "${SCRIPT_DIR}/../icons/umbra-256.png" \
    "${APP_ROOT}/io.github.femboynyx.umbra.png"
fi

tar -C "${STAGE}" -czf "${OUT}" umbra
echo "Wrote ${OUT}"
