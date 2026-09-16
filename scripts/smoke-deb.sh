#!/usr/bin/env sh
set -eu

version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)

if [ ! -d target/debian ]; then
  echo "target/debian does not exist. Run ./scripts/package-deb.sh first." >&2
  exit 1
fi

artifacts=$(find target/debian -maxdepth 1 -name '*.deb' -type f | sort)
count=$(printf '%s\n' "$artifacts" | sed '/^$/d' | wc -l | tr -d ' ')

if [ "$count" -ne 1 ]; then
  echo "Expected exactly one Debian package under target/debian, found $count." >&2
  exit 1
fi

artifact=$artifacts

[ "$(dpkg-deb --field "$artifact" Package)" = "jumpbox-typer" ] || {
  echo "Debian package has unexpected package name." >&2
  exit 1
}

[ "$(dpkg-deb --field "$artifact" Version)" = "$version" ] || {
  echo "Debian package version does not match Cargo.toml." >&2
  exit 1
}

contents=$(dpkg-deb --contents "$artifact")

printf '%s\n' "$contents" | grep -E '^-rwxr-xr-x .* \./usr/bin/jumpbox-typer$' >/dev/null || {
  echo "Debian package is missing executable /usr/bin/jumpbox-typer." >&2
  exit 1
}

for path in \
  /usr/share/applications/app.jumpbox.typer.desktop \
  /usr/share/icons/hicolor/scalable/apps/app.jumpbox.typer.svg \
  /usr/share/metainfo/app.jumpbox.typer.metainfo.xml \
  /usr/share/doc/jumpbox-typer/copyright
do
  printf '%s\n' "$contents" | grep -F " .$path" >/dev/null || {
    echo "Debian package is missing $path." >&2
    exit 1
  }
done

echo "Debian package smoke test passed: $artifact"
