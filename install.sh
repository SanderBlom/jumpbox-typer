#!/usr/bin/env sh
set -eu

PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
ICON_DIR="$PREFIX/share/icons/hicolor/scalable/apps"
APP_DIR="$PREFIX/share/applications"
METADATA_DIR="$PREFIX/share/metainfo"

mkdir -p "$BIN_DIR" "$ICON_DIR" "$APP_DIR" "$METADATA_DIR"

cargo build --release
cp target/release/jumpbox-typer "$BIN_DIR/jumpbox-typer"
cp assets/jumpbox-typer.svg "$ICON_DIR/dev.sander.jumpbox_typer.svg"
cp desktop/dev.sander.jumpbox_typer.desktop "$APP_DIR/dev.sander.jumpbox_typer.desktop"
cp desktop/dev.sander.jumpbox_typer.metainfo.xml "$METADATA_DIR/dev.sander.jumpbox_typer.metainfo.xml"

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
fi

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
fi

echo "Installed to $PREFIX"
