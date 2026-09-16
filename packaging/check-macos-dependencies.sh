#!/usr/bin/env sh
set -eu

check_path="${JUMPBOX_CHECK_PATH:-$PATH}"
missing=0

find_dependency() {
  PATH="$check_path" command -v "$1" 2>/dev/null
}

report_missing() {
  name="$1"
  command="$2"
  echo "Missing $name. Install it with Homebrew: $command" >&2
  missing=1
}

if ! find_dependency cargo >/dev/null || ! find_dependency rustc >/dev/null; then
  report_missing "Rust toolchain" "brew install rust"
fi

if pkg_config=$(find_dependency pkg-config); then
  if ! "$pkg_config" --exists gtk4; then
    report_missing "GTK 4 development files" "brew install gtk4"
  fi
  if ! "$pkg_config" --exists libadwaita-1; then
    report_missing "libadwaita development files" "brew install libadwaita"
  fi
else
  report_missing "pkg-config" "brew install pkg-config"
fi

if ! find_dependency tesseract >/dev/null; then
  report_missing "Tesseract OCR" "brew install tesseract"
fi

if ! find_dependency rsvg-convert >/dev/null; then
  report_missing "SVG icon renderer" "brew install librsvg"
fi

if [ "$missing" -ne 0 ]; then
  exit 1
fi
