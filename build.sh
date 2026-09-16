#!/usr/bin/env sh
set -eu

mkdir -p dist

host_os=$(uname -s)

if [ "$host_os" = "Darwin" ]; then
  ./packaging/check-macos-dependencies.sh
fi

cargo build --release

case "$host_os" in
  Darwin)
    bundle="dist/Jumpbox Typer.app"
    contents="$bundle/Contents"
    iconset="dist/jumpbox-typer.iconset"
    version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)

    rm -rf "$bundle" "$iconset"
    mkdir -p "$contents/MacOS" "$contents/Resources" "$iconset"
    install -m 0755 target/release/jumpbox-typer "$contents/MacOS/jumpbox-typer"
    install -m 0644 assets/jumpbox-typer.svg "$contents/Resources/jumpbox-typer.svg"

    for size in 16 32 128 256 512; do
      double_size=$((size * 2))
      rsvg-convert -w "$size" -h "$size" assets/jumpbox-typer.svg \
        -o "$iconset/icon_${size}x${size}.png"
      rsvg-convert -w "$double_size" -h "$double_size" assets/jumpbox-typer.svg \
        -o "$iconset/icon_${size}x${size}@2x.png"
    done
    iconutil -c icns "$iconset" -o "$contents/Resources/jumpbox-typer.icns"
    rm -rf "$iconset"

    sed "s/@VERSION@/$version/g" packaging/Info.plist.in >"$contents/Info.plist"
    plutil -lint "$contents/Info.plist"
    codesign --force --sign - "$bundle"
    codesign --verify --deep --strict "$bundle"
    echo "Built $bundle"
    ;;
  Linux)
    cp target/release/jumpbox-typer dist/jumpbox-typer-linux-amd64
    mkdir -p dist/assets
    cp assets/jumpbox-typer.svg dist/assets/jumpbox-typer.svg
    echo "Built dist/jumpbox-typer-linux-amd64"
    ;;
  *)
    echo "Unsupported build host: $host_os" >&2
    exit 1
    ;;
esac
