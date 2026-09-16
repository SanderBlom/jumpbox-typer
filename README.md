# Jumpbox Typer

Small tool for typing through jump hosts and remote sessions that block clipboard paste.

The app supports Linux and macOS.

For macOS setup and support information, see [README.macos.md](README.macos.md).

![Image of the app](docs/jumpbox-typer-app.png)

## What It Does

- Types pasted text into remote sessions where clipboard paste is blocked
- Extracts plain text from a screenshot of a jump host or terminal
- Helps when the remote session blocks clipboard transfer back to the host

## Install

Use the local installer:

```bash
./install.sh
```

See [BUILD.md](BUILD.md) for build and packaging notes.

The app stores typing preferences in `~/.config/jumpbox-typer/config.txt` by default, or under `XDG_CONFIG_HOME` when set. If that file is missing, the built-in defaults are used.

## Safety

The app sends real keystrokes to the focused window when the delay ends.

Later focus changes redirect the remaining text to the newly focused window.

Stop cancels after the current character sequence. It cannot remove text that the app already sent.

Test with harmless text first.
