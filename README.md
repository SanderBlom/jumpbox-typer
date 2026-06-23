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

## Usage

1. Start `jumpbox-typer`.
2. Check **System readiness**. The app checks required tools, `ydotoold`, and `/dev/uinput` access on startup. Use **Check system** after installing packages or fixing permissions.
3. Paste your script, command, or text into the textbox.
4. Set **Start delay seconds** so you have time to focus the remote session.
5. Set **Typing speed chars/sec**. Start slower for remote consoles that drop characters.
6. Optionally set **Start/stop keybind**. Examples: `F8`, `Ctrl+Alt+S`, `Shift+F9`.
7. Optionally copy an image to your clipboard, then click **Extract clipboard image text** to insert recognized text into the textbox.
8. Click **Start typing** or press the configured keybind while the app is focused.
9. Focus the target AVD, jumpbox, terminal, or remote console before the countdown ends.

The configured keybind triggers **Start typing** when idle and **Stop** while typing, as long as the app is focused. If another window is focused, stop the app from the terminal with `Ctrl+C`.

## Safety

The app sends real keystrokes to whichever window is focused when the delay ends. Test with harmless text first.
