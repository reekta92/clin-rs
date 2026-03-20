#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PKGBUILD_FILE="${ROOT_DIR}/arch/PKGBUILD"

if ! command -v makepkg >/dev/null 2>&1; then
  echo "makepkg is not installed. Install base-devel on Arch Linux first." >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is not installed or not in PATH." >&2
  exit 1
fi

if [[ ! -f "${PKGBUILD_FILE}" ]]; then
  echo "Missing PKGBUILD at ${PKGBUILD_FILE}" >&2
  exit 1
fi

PKGNAME="$(grep -m1 '^pkgname=' "${PKGBUILD_FILE}" | sed -E "s/^pkgname=['\"]?([^'\"]+)['\"]?$/\1/")"
PKGVER="$(grep -m1 '^pkgver=' "${PKGBUILD_FILE}" | sed -E "s/^pkgver=['\"]?([^'\"]+)['\"]?$/\1/")"
CARGO_VER="$(grep -m1 '^version\s*=\s*"' "${ROOT_DIR}/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')"

if [[ -z "${PKGNAME}" || -z "${PKGVER}" ]]; then
  echo "Unable to determine pkgname/pkgver from ${PKGBUILD_FILE}" >&2
  exit 1
fi

if [[ -n "${CARGO_VER}" && "${CARGO_VER}" != "${PKGVER}" ]]; then
  echo "Warning: Cargo.toml version (${CARGO_VER}) differs from PKGBUILD pkgver (${PKGVER})." >&2
fi

WORK_DIR="$(mktemp -d)"
OUT_DIR="${ROOT_DIR}/target/arch"
trap 'rm -rf "${WORK_DIR}"' EXIT

mkdir -p "${OUT_DIR}"
cp "${PKGBUILD_FILE}" "${WORK_DIR}/PKGBUILD"

tar \
  --exclude-vcs \
  --exclude='./target' \
  --exclude='./.flatpak-builder' \
  --transform "s,^\.,${PKGNAME}-${PKGVER}," \
  -czf "${WORK_DIR}/${PKGNAME}-${PKGVER}.tar.gz" \
  -C "${ROOT_DIR}" .

(cd "${WORK_DIR}" && makepkg --clean --force --nodeps)

find "${WORK_DIR}" -maxdepth 1 -type f -name '*.pkg.tar.*' -exec cp {} "${OUT_DIR}/" \;

echo "Package created in ${OUT_DIR}/"