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

## Verify a change

Run the same complete feedback loop used by continuous integration:

```bash
./scripts/verify.sh
```

The script checks Rust formatting, runs all-target tests and compilation checks, runs Clippy with warnings denied, tests the packaging workflows, and creates the host release artifact. Run it on both Linux and macOS when reproducing cross-platform verification. The repository workflow runs it on Ubuntu 24.04 and an Apple Silicon macOS 14 runner.

On Linux, the final build proves that the existing Linux executable artifact can still be produced. On macOS, `build.sh` validates bundle metadata with `plutil` and verifies the local ad-hoc signature with `codesign`. Automated tests do not request keyboard-event permission or send system-wide keystrokes.

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
- the app icon to `${XDG_DATA_HOME:-~/.local/share}/icons/hicolor/scalable/apps/dev.sander.jumpbox_typer.svg`
- the desktop entry to `${XDG_DATA_HOME:-~/.local/share}/applications/dev.sander.jumpbox_typer.desktop`
- the AppStream metadata to `${XDG_DATA_HOME:-~/.local/share}/metainfo/dev.sander.jumpbox_typer.metainfo.xml`

With `PREFIX` set, the installer copies:

- the binary to `bin/jumpbox-typer`
- the app icon to `share/icons/hicolor/scalable/apps/dev.sander.jumpbox_typer.svg`
- the desktop entry to `share/applications/dev.sander.jumpbox_typer.desktop`
- the AppStream metadata to `share/metainfo/dev.sander.jumpbox_typer.metainfo.xml`

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
