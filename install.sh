#!/usr/bin/env sh
set -eu

PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
ICON_DIR="$PREFIX/share/icons/hicolor/scalable/apps"
APP_DIR="$PREFIX/share/applications"
METADATA_DIR="$PREFIX/share/metainfo"

mkdir -p "$BIN_DIR" "$ICON_DIR" "$APP_DIR" "$METADATA_DIR"

cargo build --release

install_if_changed() {
  src="$1"
  dst="$2"

  if [ -f "$dst" ] && cmp -s "$src" "$dst"; then
    return 0
  fi

  install -m 0644 "$src" "$dst"
}

install -m 0755 target/release/jumpbox-typer "$BIN_DIR/jumpbox-typer"
install_if_changed assets/jumpbox-typer.svg "$ICON_DIR/dev.sander.jumpbox_typer.svg"
install_if_changed desktop/dev.sander.jumpbox_typer.desktop "$APP_DIR/dev.sander.jumpbox_typer.desktop"
install_if_changed desktop/dev.sander.jumpbox_typer.metainfo.xml "$METADATA_DIR/dev.sander.jumpbox_typer.metainfo.xml"

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
fi

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
fi

echo "Installed to $PREFIX"
