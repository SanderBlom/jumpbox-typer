#!/usr/bin/env sh
set -eu
umask 022

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
REPO_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)

cd "$REPO_DIR"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "macOS packaging must run on macOS." >&2
  exit 1
fi

if ! command -v hdiutil >/dev/null 2>&1; then
  echo "hdiutil is required. Run this script on macOS." >&2
  exit 1
fi

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
arch="$(uname -m)"
out_dir="target/macos"
dmg_root="$out_dir/dmg-root"
dmg_file="$out_dir/jumpbox-typer-${version}-macos-${arch}.dmg"

./build.sh

rm -rf "$dmg_root" "$dmg_file"
mkdir -p "$dmg_root"

ditto "dist/Jumpbox Typer.app" "$dmg_root/Jumpbox Typer.app"
ln -s /Applications "$dmg_root/Applications"

hdiutil create \
  -volname "Jumpbox Typer ${version}" \
  -srcfolder "$dmg_root" \
  -ov \
  -format UDZO \
  "$dmg_file"

echo "macOS disk image written to $dmg_file"
