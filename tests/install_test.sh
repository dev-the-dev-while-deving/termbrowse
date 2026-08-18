#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

payload="fixture-browse-ok"
mkdir -p "$tmp/payload"
printf '%s' "$payload" > "$tmp/payload/browse"
tar -c -z -f "$tmp/browse-0.2.0-aarch64-apple-darwin.tar.gz" -C "$tmp/payload" browse
tar -c -z -f "$tmp/browse-0.2.0-x86_64-apple-darwin.tar.gz" -C "$tmp/payload" browse
tar -c -z -f "$tmp/browse-0.2.0-x86_64-unknown-linux-musl.tar.gz" -C "$tmp/payload" browse
tar -c -z -f "$tmp/browse-0.2.0-aarch64-unknown-linux-musl.tar.gz" -C "$tmp/payload" browse

: > "$tmp/SHA256SUMS"
for f in \
  browse-0.2.0-aarch64-apple-darwin.tar.gz \
  browse-0.2.0-x86_64-apple-darwin.tar.gz \
  browse-0.2.0-x86_64-unknown-linux-musl.tar.gz \
  browse-0.2.0-aarch64-unknown-linux-musl.tar.gz
do
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$tmp" && sha256sum "$f") >> "$tmp/SHA256SUMS"
  else
    (cd "$tmp" && shasum -a 256 "$f") >> "$tmp/SHA256SUMS"
  fi
done

cat > "$tmp/release.json" <<'JSON'
{
  "tag_name": "v0.2.0",
  "assets": [
    {"name":"browse-0.2.0-aarch64-apple-darwin.tar.gz","browser_download_url":"https://example.test/browse-0.2.0-aarch64-apple-darwin.tar.gz"},
    {"name":"browse-0.2.0-x86_64-apple-darwin.tar.gz","browser_download_url":"https://example.test/browse-0.2.0-x86_64-apple-darwin.tar.gz"},
    {"name":"browse-0.2.0-x86_64-unknown-linux-musl.tar.gz","browser_download_url":"https://example.test/browse-0.2.0-x86_64-unknown-linux-musl.tar.gz"},
    {"name":"browse-0.2.0-aarch64-unknown-linux-musl.tar.gz","browser_download_url":"https://example.test/browse-0.2.0-aarch64-unknown-linux-musl.tar.gz"},
    {"name":"SHA256SUMS","browser_download_url":"https://example.test/SHA256SUMS"}
  ]
}
JSON

mkdir -p "$tmp/bin"
cat > "$tmp/bin/curl" <<EOF
#!/bin/sh
set -eu
url=""
while [ \$# -gt 0 ]; do
  case "\$1" in
    http*|https*) url="\$1" ;;
  esac
  shift
done
case "\$url" in
  *releases/latest) cat "$tmp/release.json" ;;
  *SHA256SUMS) cat "$tmp/SHA256SUMS" ;;
  *browse-0.2.0-*.tar.gz)
    name=\$(printf '%s' "\$url" | sed 's|.*/||')
    cat "$tmp/\$name"
    ;;
  *) echo "unexpected curl \$url" >&2; exit 1 ;;
esac
EOF
chmod +x "$tmp/bin/curl"

export PATH="$tmp/bin:$PATH"
export HOME="$tmp/home"
export BROWSE_INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$HOME"

sh "$root/install.sh"

test -x "$BROWSE_INSTALL_DIR/browse"
got=$(cat "$BROWSE_INSTALL_DIR/browse")
test "$got" = "$payload"
