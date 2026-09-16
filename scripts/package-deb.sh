#!/usr/bin/env sh
set -eu
umask 022

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
REPO_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)

cd "$REPO_DIR"

require_command() {
  binary="$1"
  message="$2"
  if ! command -v "$binary" >/dev/null 2>&1; then
    echo "$message" >&2
    exit 1
  fi
}

package_value() {
  key="$1"
  sed -n "s/^$key = \"\\(.*\\)\"$/\\1/p" Cargo.toml | head -n 1
}

require_command dpkg-deb "dpkg-deb is required. Install it with your system package manager."

package_name="jumpbox-typer"
package_id="dev.sander.jumpbox_typer"
version="$(package_value version)"
description="Type pasted or OCR text into remote sessions"
architecture="$(dpkg --print-architecture)"
root_dir="target/debian/${package_name}_${version}_${architecture}"
debian_dir="$root_dir/DEBIAN"
doc_dir="$root_dir/usr/share/doc/$package_name"

cargo build --release

rm -rf "$root_dir"
mkdir -p \
  "$debian_dir" \
  "$doc_dir" \
  "$root_dir/usr/bin" \
  "$root_dir/usr/share/applications" \
  "$root_dir/usr/share/icons/hicolor/scalable/apps" \
  "$root_dir/usr/share/metainfo"

cat > "$debian_dir/control" <<EOF
Package: $package_name
Version: $version
Section: utils
Priority: optional
Architecture: $architecture
Maintainer: Sander Blomvagnes
Depends: ydotool, tesseract-ocr, libgtk-4-1, libadwaita-1-0
Description: $description
 Jumpbox Typer helps paste text into terminals, jump boxes, and other remote
 sessions by sending real keystrokes. It can also extract text from clipboard
 images with OCR.
EOF

cat > "$doc_dir/copyright" <<EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Source: https://github.com/SanderBlom/jumpbox-typer

Files: *
Copyright: Sander Blomvagnes
License: MIT
EOF

install -m 0755 target/release/jumpbox-typer "$root_dir/usr/bin/jumpbox-typer"
install -m 0644 assets/jumpbox-typer.svg "$root_dir/usr/share/icons/hicolor/scalable/apps/${package_id}.svg"
install -m 0644 desktop/dev.sander.jumpbox_typer.desktop "$root_dir/usr/share/applications/${package_id}.desktop"
install -m 0644 desktop/dev.sander.jumpbox_typer.metainfo.xml "$root_dir/usr/share/metainfo/${package_id}.metainfo.xml"

dpkg-deb --root-owner-group --build "$root_dir"

echo "Debian package written to ${root_dir}.deb"
