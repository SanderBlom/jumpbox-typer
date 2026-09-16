#!/usr/bin/env sh
set -eu

if [ ! -d target/macos ]; then
  echo "target/macos does not exist. Run ./scripts/package-macos-dmg.sh first." >&2
  exit 1
fi

artifacts=$(find target/macos -maxdepth 1 -name '*.dmg' -type f | sort)
count=$(printf '%s\n' "$artifacts" | sed '/^$/d' | wc -l | tr -d ' ')

if [ "$count" -ne 1 ]; then
  echo "Expected exactly one macOS disk image under target/macos, found $count." >&2
  exit 1
fi

artifact=$artifacts
mountpoint=$(mktemp -d)

cleanup() {
  hdiutil detach "$mountpoint" >/dev/null 2>&1 || true
  rmdir "$mountpoint" 2>/dev/null || true
}
trap cleanup EXIT HUP INT TERM

hdiutil attach -readonly -nobrowse -mountpoint "$mountpoint" "$artifact" >/dev/null

app="$mountpoint/Jumpbox Typer.app"

[ -x "$app/Contents/MacOS/jumpbox-typer" ] || {
  echo "DMG is missing executable Jumpbox Typer.app/Contents/MacOS/jumpbox-typer." >&2
  exit 1
}

[ -f "$app/Contents/Info.plist" ] || {
  echo "DMG is missing Info.plist." >&2
  exit 1
}

[ -f "$app/Contents/Resources/jumpbox-typer.icns" ] || {
  echo "DMG is missing jumpbox-typer.icns." >&2
  exit 1
}

[ -L "$mountpoint/Applications" ] || {
  echo "DMG is missing Applications shortcut." >&2
  exit 1
}

echo "macOS package smoke test passed: $artifact"
