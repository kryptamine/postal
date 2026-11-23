#!/bin/sh
# Installs the latest Postal release: curl -fsSL https://postalmail.dev/install.sh | sh
# POSTAL_VERSION picks a tag (v0.1.0), POSTAL_BIN_DIR the binary's directory.
set -eu

repo=kryptamine/postal
bin_dir=${POSTAL_BIN_DIR:-$HOME/.local/bin}
plugin=${XDG_DATA_HOME:-$HOME/.local/share}/postal/nvim

fail() {
  echo "postal: $*" >&2
  exit 1
}

case $(uname -s) in
  Darwin) os=apple-darwin ;;
  Linux) os=unknown-linux-gnu ;;
  *) fail "no release for $(uname -s); build from source: https://github.com/$repo" ;;
esac

case $(uname -m) in
  arm64 | aarch64) arch=aarch64 ;;
  x86_64 | amd64) arch=x86_64 ;;
  *) fail "no release for $(uname -m); build from source: https://github.com/$repo" ;;
esac

name=postal-$arch-$os
release=${POSTAL_VERSION:+download/$POSTAL_VERSION}
url=https://github.com/$repo/releases/${release:-latest/download}/$name.tar.gz

fetch() {
  if command -v curl >/dev/null; then
    curl -fsSL -o "$2" "$1"
  elif command -v wget >/dev/null; then
    wget -qO "$2" "$1"
  else
    fail "needs curl or wget"
  fi
}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "postal: downloading $url"
fetch "$url" "$tmp/$name.tar.gz" || fail "download failed"
fetch "$url.sha256" "$tmp/$name.tar.gz.sha256" || fail "checksum download failed"

# The checksum file names the archive, so it is checked from the archive's directory.
if command -v sha256sum >/dev/null; then
  (cd "$tmp" && sha256sum -c --status "$name.tar.gz.sha256") || fail "checksum mismatch"
else
  (cd "$tmp" && shasum -a 256 -c -s "$name.tar.gz.sha256") || fail "checksum mismatch"
fi

tar -xzf "$tmp/$name.tar.gz" -C "$tmp"

mkdir -p "$bin_dir"
# A move replaces the file a running postal still has open, rather than writing into it.
cp "$tmp/$name/postal" "$bin_dir/postal.new"
chmod 755 "$bin_dir/postal.new"
mv "$bin_dir/postal.new" "$bin_dir/postal"

# The plugin is copied whole, then swapped in, so no Neovim loads half of one.
rm -rf "$plugin.new" && mkdir -p "$plugin.new"
cp -R "$tmp/$name/nvim/lua" "$plugin.new/"
rm -rf "$plugin" && mv "$plugin.new" "$plugin"

echo "postal: $bin_dir/postal"
echo "plugin: $plugin"
case :$PATH: in
  *:"$bin_dir":*) ;;
  *) echo "postal: add $bin_dir to your PATH" ;;
esac
