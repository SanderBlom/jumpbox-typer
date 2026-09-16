# Jumpbox Typer for macOS

Jumpbox Typer types prepared text into focused applications that block clipboard paste.

macOS support remains experimental until a remote-session client passes manual validation.

Jumpbox Typer supports macOS 14 or later on Apple Silicon.

## Safety

Jumpbox Typer sends real keystrokes to the focused window after the delay ends.

A later focus change redirects the remaining text to the new focused window.

The Stop control cancels after the current character sequence. It cannot remove text that the application already sent.

Always test with harmless text before you type commands or sensitive text.

## Requirements

The local build requires these tools and libraries:

- Rust
- `pkg-config`
- GTK 4
- libadwaita
- Tesseract OCR
- librsvg

Install the required Homebrew packages:

```bash
brew install rust pkg-config gtk4 libadwaita tesseract librsvg
```

The build script checks these dependencies. It does not install or change Homebrew packages.

## Build the application

1. Open Terminal in the repository root.

2. Run the build script.

   ```bash
   ./build.sh
   ```

3. Find the application bundle at this path.

   ```text
   dist/Jumpbox Typer.app
   ```

The build script creates the icon and application metadata. It also adds an ad-hoc code signature.

The script validates the property list and verifies the code signature.

## Install the application

1. Run the local installer.

   ```bash
   ./install.sh
   ```

2. Find the installed application at this path.

   ```text
   ~/Applications/Jumpbox Typer.app
   ```

Set `APP_DIR` if you need a different application directory:

```bash
APP_DIR=/Applications ./install.sh
```

The installer replaces only `Jumpbox Typer.app` in the selected directory. It does not change other applications or user data.

## Grant keyboard-event permission

Jumpbox Typer checks keyboard-event permission when it starts. It does not request permission during startup.

The Start control remains disabled when permission is not available.

1. Start Jumpbox Typer from Finder.

2. Select **Check System**.

3. Grant the requested permission in macOS System Settings.

4. Return to Jumpbox Typer.

5. Select **Check System** again if the status does not change.

macOS can request permission again after you replace an ad-hoc signed build.

## Select a keyboard layout

Select the same layout in Jumpbox Typer and the active macOS input source.

The US layout provides physical key mappings for printable US ASCII characters.

The Norwegian layout provides physical key mappings for Norwegian letters and supported layout-specific symbols.

Other characters use a best-effort Core Graphics Unicode event. The event supports UTF-16 and surrogate pairs.

An application or remote-session client can reject or change Unicode text. Jumpbox Typer does not guarantee arbitrary Unicode input.

## Use clipboard-image OCR

Tesseract must remain installed on the Mac. The application does not include Tesseract in its bundle.

The OCR button accepts direct clipboard images. It also accepts one local image file copied through Raycast or Finder.

Copy only one image file. The OCR process reads the source file and does not delete or change it.

Jumpbox Typer searches the inherited `PATH` first. It then searches these Homebrew locations:

- `/opt/homebrew/bin`
- `/usr/local/bin`

The system check and OCR process use the same Tesseract executable.

Test OCR after you start the installed application from Finder. Finder can provide a different environment from Terminal.

## Verify a change

Run the complete verification script:

```bash
./scripts/verify.sh
```

The script checks Rust formatting, tests, compilation, Clippy, packaging, bundle metadata, and the ad-hoc signature.

Automated tests do not request keyboard-event permission. They also do not send system-wide keystrokes.

Complete these manual checks after automated verification:

1. Start the installed application from Finder.

2. Type harmless text into TextEdit.

3. Type the same text into Terminal.

4. Test the delay, speed, Enter pause, progress, Stop control, and focus changes.

5. Test clipboard-image OCR from the Finder-started application.

6. Test a real remote-session client before you claim compatibility.

Record the macOS version, hardware, application revision, client version, input source, test text, and observed limits.

## Configuration

Jumpbox Typer stores its preferences in this file by default:

```text
~/.config/jumpbox-typer/config.txt
```

The application uses `XDG_CONFIG_HOME` when that environment variable has a value.

## Support limits

The application bundle uses Homebrew GTK and libadwaita libraries from the build Mac.

The bundle has an ad-hoc signature. It does not have a Developer ID signature or Apple notarization.

The bundle is not a portable public release. Use it locally on the Mac that built it.

Jumpbox Typer does not currently support Intel Macs, App Store distribution, or a native macOS interface.

No remote-session client has passed the manual compatibility procedure. Do not infer support for an untested client.
