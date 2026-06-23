# Jumpbox Typer

A small Ubuntu/Linux GTK 4 desktop app for typing pasted text into environments where the clipboard is blocked, such as AVD sessions, jumpboxes, web consoles, and remote server terminals.

Paste text into the app, set a start delay and typing speed, click **Start typing**, then focus the remote window before the countdown finishes.

## Runtime Requirements

- Ubuntu/Linux desktop
- Wayland or X11
- `ydotool`
- `ydotoold` running with access to `/dev/uinput`
- `tesseract-ocr` for OCR

Install runtime dependency:

```bash
sudo apt install ydotool tesseract-ocr
```

`ydotool` works through Linux `uinput`, so it is not tied to Xorg. The daemon needs permission to create a virtual keyboard device. On Ubuntu, package setup can vary, so check the installed service name with:

```bash
systemctl list-unit-files | grep ydotool
```

Then enable/start the matching service if the package provides one.

If `ydotoold` fails with `failed to open uinput device: Permission denied`, your user needs access to `/dev/uinput`. On Ubuntu this is commonly:

```bash
sudo usermod -aG input $USER
```

Then log out and back in so the new group membership applies. This grants broad input-device access, so only do it on a machine where that tradeoff is acceptable.

## Build Requirements

- Rust toolchain
- GTK 4 development package
- Libadwaita development package
- `pkg-config`

On Ubuntu:

```bash
sudo apt install cargo rustc gcc pkg-config libgtk-4-dev libadwaita-1-dev
```

## Run From Source

```bash
cargo run
```

## Build Binary

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

The build script writes `dist/jumpbox-typer-linux-amd64`.

## Install

Install into your user prefix:

```bash
./install.sh
```

Or install to a custom prefix:

```bash
PREFIX=/usr/local ./install.sh
```

The installer copies:

- the binary to `bin/jumpbox-typer`
- the app icon to `share/icons/hicolor/scalable/apps/dev.sander.jumpbox_typer.svg`
- the desktop entry to `share/applications/dev.sander.jumpbox_typer.desktop`
- the AppStream metadata to `share/metainfo/dev.sander.jumpbox_typer.metainfo.xml`

## Usage

1. Start `jumpbox-typer`.
2. Check **System readiness**. The app checks required tools, `ydotoold`, and `/dev/uinput` access on startup. Use **Check system** after installing packages or fixing permissions.
3. Paste your script, command, or text into the textbox.
4. Set **Start delay seconds** so you have time to focus the remote session.
5. Set **Typing speed chars/sec**. Start slower for remote consoles that drop characters.
6. Optionally copy an image to your clipboard, then click **Extract clipboard image text** to insert recognized text into the textbox.
7. Click **Start typing**.
8. Focus the target AVD, jumpbox, terminal, or remote console before the countdown ends.

Use **Stop** from the app window if you need to cancel a running job.

## Safety

The app sends real keystrokes to whichever window is focused when the delay ends. Test with harmless text first.
