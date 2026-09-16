#!/usr/bin/env sh
set -eu

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

test_darwin_build_creates_standard_bundle() {
  test_tmp=$(mktemp -d)
  trap 'rm -rf "$test_tmp"' EXIT HUP INT TERM

  project="$test_tmp/project"
  fake_bin="$test_tmp/bin"
  mkdir -p "$project/assets" "$project/packaging" "$fake_bin"
  cp build.sh Cargo.toml Cargo.lock "$project/"
  cp assets/jumpbox-typer.svg "$project/assets/"
  cp packaging/Info.plist.in packaging/check-macos-dependencies.sh "$project/packaging/"

  printf '%s\n' \
    '#!/usr/bin/env sh' \
    'case "${1:-}" in' \
    '  -s) echo Darwin ;;' \
    '  -m) echo arm64 ;;' \
    '  *) echo Darwin ;;' \
    'esac' >"$fake_bin/uname"
  printf '%s\n' \
    '#!/usr/bin/env sh' \
    'mkdir -p target/release' \
    "printf '#!/usr/bin/env sh\\nexit 0\\n' > target/release/jumpbox-typer" \
    'chmod +x target/release/jumpbox-typer' >"$fake_bin/cargo"
  printf '%s\n' '#!/usr/bin/env sh' 'exit 0' >"$fake_bin/rustc"
  printf '%s\n' '#!/usr/bin/env sh' 'exit 0' >"$fake_bin/pkg-config"
  printf '%s\n' '#!/usr/bin/env sh' 'exit 0' >"$fake_bin/tesseract"
  printf '%s\n' \
    '#!/usr/bin/env sh' \
    'output=' \
    'while [ "$#" -gt 0 ]; do' \
    '  if [ "$1" = "-o" ]; then shift; output=$1; fi' \
    '  shift' \
    'done' \
    ': >"$output"' >"$fake_bin/rsvg-convert"
  printf '%s\n' \
    '#!/usr/bin/env sh' \
    'output=' \
    'while [ "$#" -gt 0 ]; do' \
    '  if [ "$1" = "-o" ]; then shift; output=$1; fi' \
    '  shift' \
    'done' \
    ': >"$output"' >"$fake_bin/iconutil"
  printf '%s\n' '#!/usr/bin/env sh' 'exit 0' >"$fake_bin/plutil"
  printf '%s\n' '#!/usr/bin/env sh' 'exit 0' >"$fake_bin/codesign"
  chmod +x "$fake_bin/uname" "$fake_bin/cargo" "$fake_bin/rustc" \
    "$fake_bin/pkg-config" "$fake_bin/tesseract" "$fake_bin/rsvg-convert" \
    "$fake_bin/iconutil" "$fake_bin/plutil" "$fake_bin/codesign" \
    "$project/packaging/check-macos-dependencies.sh"

  (
    cd "$project"
    PATH="$fake_bin:$PATH" ./build.sh
  )

  bundle="$project/dist/Jumpbox Typer.app"
  [ -x "$bundle/Contents/MacOS/jumpbox-typer" ] || fail "Darwin build did not bundle executable"
  [ -f "$bundle/Contents/Info.plist" ] || fail "Darwin build did not create Info.plist"
  [ -f "$bundle/Contents/Resources/jumpbox-typer.icns" ] || fail "Darwin build did not create icon"
  grep -F '<string>app.jumpbox.typer</string>' "$bundle/Contents/Info.plist" >/dev/null || \
    fail "Darwin bundle has wrong identifier"
  grep -F '<string>14.0</string>' "$bundle/Contents/Info.plist" >/dev/null || \
    fail "Darwin bundle has wrong minimum macOS version"
}

test_darwin_install_copies_bundle_to_override() {
  test_tmp=$(mktemp -d)
  trap 'rm -rf "$test_tmp"' EXIT HUP INT TERM

  project="$test_tmp/project"
  fake_bin="$test_tmp/bin"
  applications="$test_tmp/Applications"
  mkdir -p "$project/dist/Jumpbox Typer.app/Contents/MacOS" "$fake_bin" "$applications/Other.app"
  cp install.sh "$project/"
  printf '%s\n' '#!/usr/bin/env sh' 'exit 0' >"$project/build.sh"
  printf '%s\n' '#!/usr/bin/env sh' 'echo app' >"$project/dist/Jumpbox Typer.app/Contents/MacOS/jumpbox-typer"
  printf '%s\n' '#!/usr/bin/env sh' 'echo Darwin' >"$fake_bin/uname"
  printf '%s\n' \
    '#!/usr/bin/env sh' \
    'cp -R "$1" "$2"' >"$fake_bin/ditto"
  chmod +x "$project/build.sh" "$fake_bin/uname" "$fake_bin/ditto"

  (
    cd "$project"
    PATH="$fake_bin:$PATH" APP_DIR="$applications" ./install.sh
  )

  [ -f "$applications/Jumpbox Typer.app/Contents/MacOS/jumpbox-typer" ] || \
    fail "macOS installer did not copy bundle to APP_DIR"
  [ -d "$applications/Other.app" ] || fail "macOS installer removed unrelated application"
}

test_darwin_build_reports_missing_tesseract() {
  test_tmp=$(mktemp -d)
  trap 'rm -rf "$test_tmp"' EXIT HUP INT TERM

  project="$test_tmp/project"
  fake_bin="$test_tmp/bin"
  mkdir -p "$project/assets" "$project/packaging" "$fake_bin"
  cp build.sh Cargo.toml Cargo.lock "$project/"
  cp assets/jumpbox-typer.svg "$project/assets/"
  cp packaging/Info.plist.in packaging/check-macos-dependencies.sh "$project/packaging/"

  printf '%s\n' '#!/usr/bin/env sh' 'echo Darwin' >"$fake_bin/uname"
  printf '%s\n' \
    '#!/usr/bin/env sh' \
    'mkdir -p target/release' \
    ': > target/release/jumpbox-typer' >"$fake_bin/cargo"
  printf '%s\n' '#!/usr/bin/env sh' 'exit 0' >"$fake_bin/rustc"
  printf '%s\n' '#!/usr/bin/env sh' 'exit 0' >"$fake_bin/pkg-config"
  printf '%s\n' '#!/usr/bin/env sh' 'exit 0' >"$fake_bin/rsvg-convert"
  chmod +x "$fake_bin/uname" "$fake_bin/cargo" "$fake_bin/rustc" \
    "$fake_bin/pkg-config" "$fake_bin/rsvg-convert" \
    "$project/packaging/check-macos-dependencies.sh"

  if output=$(
    cd "$project"
    PATH="$fake_bin:$PATH" JUMPBOX_CHECK_PATH="$fake_bin" ./build.sh 2>&1
  ); then
    fail "macOS build succeeded without Tesseract"
  fi

  echo "$output" | grep -F "brew install tesseract" >/dev/null || \
    fail "missing Tesseract guidance omitted Homebrew command"
}

test_linux_build_and_install_stay_unchanged() {
  test_tmp=$(mktemp -d)
  trap 'rm -rf "$test_tmp"' EXIT HUP INT TERM

  project="$test_tmp/project"
  fake_bin="$test_tmp/bin"
  prefix="$test_tmp/prefix"
  mkdir -p "$project/assets" "$project/packaging/linux" "$fake_bin"
  cp build.sh install.sh Cargo.toml Cargo.lock "$project/"
  cp assets/jumpbox-typer.svg "$project/assets/"
  cp packaging/linux/app.jumpbox.typer.desktop \
    packaging/linux/app.jumpbox.typer.metainfo.xml "$project/packaging/linux/"

  printf '%s\n' '#!/usr/bin/env sh' 'echo Linux' >"$fake_bin/uname"
  printf '%s\n' \
    '#!/usr/bin/env sh' \
    'mkdir -p target/release' \
    ': > target/release/jumpbox-typer' \
    'chmod +x target/release/jumpbox-typer' >"$fake_bin/cargo"
  chmod +x "$fake_bin/uname" "$fake_bin/cargo"

  (
    cd "$project"
    PATH="$fake_bin:$PATH" ./build.sh
    PATH="$fake_bin:$PATH" PREFIX="$prefix" ./install.sh
  )

  [ -x "$prefix/bin/jumpbox-typer" ] || fail "Linux installer changed binary destination"
  [ -f "$prefix/share/applications/app.jumpbox.typer.desktop" ] || \
    fail "Linux installer changed desktop-entry destination"
  [ -f "$project/dist/jumpbox-typer-linux-amd64" ] || fail "Linux build artifact changed"
}

test_debian_package_smoke_checks_expected_payload() {
  test_tmp=$(mktemp -d)
  trap 'rm -rf "$test_tmp"' EXIT HUP INT TERM

  project="$test_tmp/project"
  fake_bin="$test_tmp/bin"
  mkdir -p "$project/scripts" "$project/target/debian" "$fake_bin"
  cp Cargo.toml "$project/"
  cp scripts/smoke-deb.sh "$project/scripts/"
  : >"$project/target/debian/jumpbox-typer_0.1.0_amd64.deb"

  printf '%s\n' \
    '#!/usr/bin/env sh' \
    'case "$1" in' \
    '  --field)' \
    '    case "$3" in' \
    '      Package) echo jumpbox-typer ;;' \
    '      Version) sed -n '"'"'s/^version = "\([^"]*\)"/\1/p'"'"' Cargo.toml | head -n 1 ;;' \
    '      *) exit 1 ;;' \
    '    esac ;;' \
    '  --contents)' \
    '    cat <<EOF' \
    '-rwxr-xr-x root/root 0 2026-01-01 00:00 ./usr/bin/jumpbox-typer' \
    '-rw-r--r-- root/root 0 2026-01-01 00:00 ./usr/share/applications/app.jumpbox.typer.desktop' \
    '-rw-r--r-- root/root 0 2026-01-01 00:00 ./usr/share/icons/hicolor/scalable/apps/app.jumpbox.typer.svg' \
    '-rw-r--r-- root/root 0 2026-01-01 00:00 ./usr/share/metainfo/app.jumpbox.typer.metainfo.xml' \
    '-rw-r--r-- root/root 0 2026-01-01 00:00 ./usr/share/doc/jumpbox-typer/copyright' \
    'EOF' \
    '    ;;' \
    '  *) exit 1 ;;' \
    'esac' >"$fake_bin/dpkg-deb"
  chmod +x "$fake_bin/dpkg-deb" "$project/scripts/smoke-deb.sh"

  (
    cd "$project"
    PATH="$fake_bin:$PATH" ./scripts/smoke-deb.sh
  )
}

test_macos_dmg_smoke_checks_expected_payload() {
  test_tmp=$(mktemp -d)
  trap 'rm -rf "$test_tmp"' EXIT HUP INT TERM

  project="$test_tmp/project"
  fake_bin="$test_tmp/bin"
  mkdir -p "$project/scripts" "$project/target/macos" "$fake_bin"
  cp scripts/smoke-macos-dmg.sh "$project/scripts/"
  : >"$project/target/macos/jumpbox-typer-0.1.0-macos-arm64.dmg"

  printf '%s\n' \
    '#!/usr/bin/env sh' \
    'case "$1" in' \
    '  attach)' \
    '    mountpoint=' \
    '    while [ "$#" -gt 0 ]; do' \
    '      if [ "$1" = "-mountpoint" ]; then shift; mountpoint=$1; fi' \
    '      shift' \
    '    done' \
    '    mkdir -p "$mountpoint/Jumpbox Typer.app/Contents/MacOS" "$mountpoint/Jumpbox Typer.app/Contents/Resources"' \
    '    printf "#!/usr/bin/env sh\nexit 0\n" >"$mountpoint/Jumpbox Typer.app/Contents/MacOS/jumpbox-typer"' \
    '    chmod +x "$mountpoint/Jumpbox Typer.app/Contents/MacOS/jumpbox-typer"' \
    '    : >"$mountpoint/Jumpbox Typer.app/Contents/Info.plist"' \
    '    : >"$mountpoint/Jumpbox Typer.app/Contents/Resources/jumpbox-typer.icns"' \
    '    ln -s /Applications "$mountpoint/Applications"' \
    '    ;;' \
    '  detach)' \
    '    rm -rf "$2" ;;' \
    '  *) exit 1 ;;' \
    'esac' >"$fake_bin/hdiutil"
  chmod +x "$fake_bin/hdiutil" "$project/scripts/smoke-macos-dmg.sh"

  (
    cd "$project"
    PATH="$fake_bin:$PATH" ./scripts/smoke-macos-dmg.sh
  )
}

test_darwin_build_creates_standard_bundle
test_darwin_install_copies_bundle_to_override
test_darwin_build_reports_missing_tesseract
test_linux_build_and_install_stay_unchanged
test_debian_package_smoke_checks_expected_payload
test_macos_dmg_smoke_checks_expected_payload
echo "packaging tests passed"
