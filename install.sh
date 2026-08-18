#!/bin/sh
set -eu

REPO="${BROWSE_REPO:-dev-the-dev-while-deving/termbrowse}"
INSTALL_DIR="${BROWSE_INSTALL_DIR:-$HOME/.local/bin}"
UA="${BROWSE_UA:-browse-installer}"
API_URL="${BROWSE_API_URL:-https://api.github.com/repos/${REPO}/releases/latest}"

detect_target() {
  sys=$(uname -s)
  mach=$(uname -m)
  case "${sys}:${mach}" in
    Darwin:arm64) echo aarch64-apple-darwin ;;
    Darwin:x86_64) echo x86_64-apple-darwin ;;
    Linux:x86_64) echo x86_64-unknown-linux-musl ;;
    Linux:aarch64|Linux:arm64) echo aarch64-unknown-linux-musl ;;
    *)
      echo "unsupported platform: ${sys} ${mach}" >&2
      exit 1
      ;;
  esac
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

json_url() {
  needle=$1
  printf '%s' "$json" | tr '"' '\n' | grep -F "$needle" | grep -E '^https?://' | head -n 1
}

need_cmd curl
need_cmd tar
need_cmd uname
need_cmd mktemp

target=$(detect_target)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

json=$(curl -fsSL -A "$UA" "$API_URL") || {
  echo "no release found; tag a version (vX.Y.Z) first" >&2
  exit 1
}

tag=$(printf '%s' "$json" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
if [ -z "$tag" ]; then
  echo "no release found; tag a version (vX.Y.Z) first" >&2
  exit 1
fi
version=$(printf '%s' "$tag" | sed 's/^[vV]//')
asset="browse-${version}-${target}.tar.gz"

asset_url=$(json_url "$asset")
sums_url=$(json_url "SHA256SUMS")

if [ -z "$asset_url" ] || [ -z "$sums_url" ]; then
  echo "no release found; tag a version (vX.Y.Z) first" >&2
  exit 1
fi

curl -fsSL -A "$UA" "$asset_url" > "$work/$asset"
curl -fsSL -A "$UA" "$sums_url" > "$work/SHA256SUMS"

expected=$(awk -v n="$asset" '$2 == n || $2 == "*"n { print $1; exit }' "$work/SHA256SUMS")
if [ -z "$expected" ]; then
  echo "checksum mismatch, aborting" >&2
  exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
  got=$(sha256sum "$work/$asset" | awk '{print $1}')
else
  got=$(shasum -a 256 "$work/$asset" | awk '{print $1}')
fi

expected=$(printf '%s' "$expected" | tr 'A-F' 'a-f')
got=$(printf '%s' "$got" | tr 'A-F' 'a-f')

if [ "$got" != "$expected" ]; then
  echo "checksum mismatch, aborting" >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR"
tar -x -z -f "$work/$asset" -C "$work" browse
chmod 755 "$work/browse"
mv "$work/browse" "$INSTALL_DIR/browse.new"
mv "$INSTALL_DIR/browse.new" "$INSTALL_DIR/browse"

echo "installed browse ${version} -> ${INSTALL_DIR}/browse"

case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo ""
    echo "Add this to your shell profile, then open a new terminal:"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    ;;
esac
