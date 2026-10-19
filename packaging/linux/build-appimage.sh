#!/usr/bin/env bash
# Build an Umbra AppImage from a Flutter Linux release bundle.
#
# Usage:
#   packaging/linux/build-appimage.sh <flutter-bundle-dir> [output.AppImage]
#
# <flutter-bundle-dir> is typically:
#   app/build/linux/x64/release/bundle
#
# Requires: appimagetool on PATH (or set APPIMAGETOOL).
# Optional env:
#   UMBRA_VERSION   Version for default output name (default: 0.0.0-dev)
#   DEST_DIR        Output directory (default: cwd)
#   APPIMAGETOOL    Path to appimagetool binary
#   ARCH            AppImage arch (default: x86_64)
#
# Exit codes: 0 success, 1 usage/input error, 2 missing tool, 3 build failure.
set -euo pipefail

usage() {
  echo "Usage: $0 <flutter-bundle-dir> [output.AppImage]" >&2
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

APPIMAGETOOL_BIN="${APPIMAGETOOL:-appimagetool}"
if ! command -v "${APPIMAGETOOL_BIN}" >/dev/null 2>&1; then
  echo "error: appimagetool not found (set APPIMAGETOOL or install it)" >&2
  exit 2
fi

VERSION="${UMBRA_VERSION:-0.0.0-dev}"
DEST_DIR="${DEST_DIR:-.}"
ARCH="${ARCH:-x86_64}"
mkdir -p "${DEST_DIR}"

DEFAULT_OUT="${DEST_DIR}/Umbra-${VERSION}-${ARCH}.AppImage"
OUT="${2:-${DEFAULT_OUT}}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ICONS_DIR="${SCRIPT_DIR}/../icons"

STAGE="$(mktemp -d)"
cleanup() { rm -rf "${STAGE}"; }
trap cleanup EXIT

APPDIR="${STAGE}/Umbra.AppDir"
mkdir -p \
  "${APPDIR}/usr/bin" \
  "${APPDIR}/usr/lib" \
  "${APPDIR}/usr/share/applications" \
  "${APPDIR}/usr/share/metainfo" \
  "${APPDIR}/usr/share/icons/hicolor/256x256/apps"

# Copy Flutter bundle under usr/lib/umbra (keeps lib/ and data/ layout intact).
mkdir -p "${APPDIR}/usr/lib/umbra"
cp -a "${BUNDLE_DIR}/." "${APPDIR}/usr/lib/umbra/"

# Resolve the Flutter binary name (usually "umbra").
FLUTTER_BIN=""
if [[ -x "${APPDIR}/usr/lib/umbra/umbra" ]]; then
  FLUTTER_BIN="umbra"
else
  FLUTTER_BIN="$(find "${APPDIR}/usr/lib/umbra" -maxdepth 1 -type f -executable -printf '%f\n' | head -n1 || true)"
fi
if [[ -z "${FLUTTER_BIN}" ]]; then
  echo "error: no executable found in Flutter bundle" >&2
  exit 1
fi

# Thin wrapper so Exec=umbra works from the desktop file.
cat > "${APPDIR}/usr/bin/umbra" << WRAP
#!/usr/bin/env bash
set -euo pipefail
HERE="\$(cd "\$(dirname "\$(readlink -f "\$0")" )/.." && pwd)"
export LD_LIBRARY_PATH="\${HERE}/lib/umbra/lib:\${LD_LIBRARY_PATH:-}"
exec "\${HERE}/lib/umbra/${FLUTTER_BIN}" "\$@"
WRAP
chmod 0755 "${APPDIR}/usr/bin/umbra"

# AppRun (required by AppImage)
cat > "${APPDIR}/AppRun" << 'APPRUN'
#!/usr/bin/env bash
set -euo pipefail
HERE="$(dirname "$(readlink -f "$0")")"
export LD_LIBRARY_PATH="${HERE}/usr/lib/umbra/lib:${LD_LIBRARY_PATH:-}"
# Prefer the Flutter binary directly for AppRun.
if [[ -x "${HERE}/usr/lib/umbra/umbra" ]]; then
  exec "${HERE}/usr/lib/umbra/umbra" "$@"
fi
BIN="$(find "${HERE}/usr/lib/umbra" -maxdepth 1 -type f -executable | head -n1)"
exec "${BIN}" "$@"
APPRUN
chmod 0755 "${APPDIR}/AppRun"

install -m 0644 "${SCRIPT_DIR}/umbra.desktop" \
  "${APPDIR}/usr/share/applications/umbra.desktop"
# AppImage expects a desktop file at AppDir root with Icon= matching a root icon.
cp "${APPDIR}/usr/share/applications/umbra.desktop" "${APPDIR}/umbra.desktop"
# Point Icon to the AppDir-root icon name.
sed -i 's|^Icon=.*|Icon=umbra|' "${APPDIR}/umbra.desktop"

install -m 0644 "${SCRIPT_DIR}/io.github.femboynyx.umbra.metainfo.xml" \
  "${APPDIR}/usr/share/metainfo/io.github.femboynyx.umbra.metainfo.xml"

ICON_SRC="${ICONS_DIR}/umbra-256.png"
if [[ ! -f "${ICON_SRC}" ]]; then
  echo "error: missing icon ${ICON_SRC}" >&2
  exit 1
fi
install -m 0644 "${ICON_SRC}" \
  "${APPDIR}/usr/share/icons/hicolor/256x256/apps/io.github.femboynyx.umbra.png"
install -m 0644 "${ICON_SRC}" "${APPDIR}/umbra.png"

if ! "${APPIMAGETOOL_BIN}" "${APPDIR}" "${OUT}"; then
  echo "error: appimagetool failed" >&2
  exit 3
fi
chmod 0755 "${OUT}" || true
echo "Wrote ${OUT}"
