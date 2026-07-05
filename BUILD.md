# Build

## Requirements

- Rust toolchain
- GTK 4 development package
- Libadwaita development package
- `pkg-config`

On Ubuntu:

```bash
sudo apt install cargo rustc gcc pkg-config libgtk-4-dev libadwaita-1-dev
```

## Build From Source

```bash
cargo build --release
```

The binary will be at:

```bash
target/release/jumpbox-typer
```

Or run:

```bash
./build.sh
```

The build script writes `dist/jumpbox-typer-linux-amd64` and packages the icon asset under `dist/assets/`.

## Install

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
