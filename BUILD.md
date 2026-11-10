# Build

## Linux requirements

- Rust toolchain
- GTK 4 development package
- Libadwaita development package
- `pkg-config`
- Tesseract OCR

On Ubuntu:

```bash
sudo apt install cargo rustc gcc pkg-config libgtk-4-dev libadwaita-1-dev tesseract-ocr
```

## macOS requirements

The local macOS bundle supports macOS 14 or later on Apple Silicon.

Building requires Rust, package configuration through `pkg-config`, GTK 4, libadwaita, Tesseract, and librsvg for icon generation.

Install the required Homebrew packages:

```bash
brew install rust pkg-config gtk4 libadwaita tesseract librsvg
```

The build script checks each dependency. The script does not install or change Homebrew packages.

## Build from source

```bash
./build.sh
```

On Linux, the script creates these files:

```bash
dist/jumpbox-typer-linux-amd64
dist/assets/jumpbox-typer.svg
```

On macOS, the script creates this application bundle:

```bash
dist/Jumpbox Typer.app
```

The script generates the application icon from `assets/jumpbox-typer.svg`. It validates the property list and ad-hoc signs the bundle.

The macOS bundle links to the Homebrew libraries on the build Mac. It is not a portable, notarized release.

Tesseract stays outside the bundle. The application searches its inherited `PATH`, `/opt/homebrew/bin`, and `/usr/local/bin`.

## Build release packages

On Ubuntu 26.04:

```bash
./scripts/package-deb.sh
```

The Debian package is written to `target/debian/`.

On macOS:

```bash
./scripts/package-macos-dmg.sh
```

The disk image is written to `target/macos/`.

## Verify a change

Run the same complete feedback loop used by continuous integration:

```bash
./scripts/verify.sh
```

The script checks Rust formatting, runs all-target tests and compilation checks, runs Clippy with warnings denied, tests the packaging workflows, and creates the host release artifact. Run it on both Linux and macOS when reproducing cross-platform verification. The repository workflow runs it on Ubuntu 24.04 and an Apple Silicon macOS 15 runner.

On Linux, the final build proves that the existing Linux executable artifact can still be produced. On macOS, `build.sh` validates bundle metadata with `plutil` and verifies the local ad-hoc signature with `codesign`. Automated tests do not request keyboard-event permission or send system-wide keystrokes.

Linux CI runs verification under Xvfb with a private D-Bus session so the GTK
lifecycle scenarios execute rather than skip. To exercise the same checks locally,
install `xvfb`, `xauth`, and `dbus-x11`, then run:

```bash
dbus-run-session -- xvfb-run -a env GDK_BACKEND=x11 ./scripts/verify.sh
```

Finder-launched GUI and OCR behavior still require manual verification because Finder uses the installed application identity and can provide a different environment from a terminal launch.

## Install on Linux

```bash
./install.sh
```

Or install to a custom prefix:

```bash
PREFIX=/usr/local ./install.sh
```

Without `PREFIX`, the installer follows the user XDG layout and copies:

- the binary to `~/.local/bin/jumpbox-typer`
- the app icon to `${XDG_DATA_HOME:-~/.local/share}/icons/hicolor/scalable/apps/app.jumpbox.typer.svg`
- the desktop entry to `${XDG_DATA_HOME:-~/.local/share}/applications/app.jumpbox.typer.desktop`
- the AppStream metadata to `${XDG_DATA_HOME:-~/.local/share}/metainfo/app.jumpbox.typer.metainfo.xml`

With `PREFIX` set, the installer copies:

- the binary to `bin/jumpbox-typer`
- the app icon to `share/icons/hicolor/scalable/apps/app.jumpbox.typer.svg`
- the desktop entry to `share/applications/app.jumpbox.typer.desktop`
- the AppStream metadata to `share/metainfo/app.jumpbox.typer.metainfo.xml`

## Install on macOS

Run the local installer:

```bash
./install.sh
```

The installer builds the bundle and copies it to `~/Applications/Jumpbox Typer.app`.

Set `APP_DIR` to use a different application directory:

```bash
APP_DIR=/Applications ./install.sh
```

The installer replaces only `Jumpbox Typer.app` in the selected directory. It does not change other applications or user data.

macOS can require Accessibility permission again after you replace an ad-hoc signed development build. Use Check System to request permission.

## GitHub releases

Publishing a GitHub release runs [release-packages.yml](.github/workflows/release-packages.yml).

Before publishing, update the package version in both `Cargo.toml` and `Cargo.lock`
and tag that commit with the same version (an optional `v` prefix is accepted).
For example, version `1.0.0-beta-2` must use tag `1.0.0-beta-2` or
`v1.0.0-beta-2`. The workflow attaches packages to the published release; it does
not create the release itself. macOS packaging runs on an Apple Silicon macOS 15
runner; the retired `macos-14` runner is not used.

The release workflow builds and uploads:

- a `.deb` package from Ubuntu 26.04
- a `.dmg` disk image from Apple Silicon macOS
