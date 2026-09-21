# Releasing Jumpbox Typer

Use this checklist for each public release.

## Prepare

- Start from an up-to-date `main` branch.
- Choose the version, for example `1.0.0`.
- Update `Cargo.toml` package `version`.
- Update AppStream release metadata in `packaging/linux/app.jumpbox.typer.metainfo.xml`.
- Confirm `rust-toolchain.toml`, `Cargo.toml` `rust-version`, and CI all pin the same Rust toolchain.
- Run `./scripts/verify.sh`.

## Smoke Test

- Build and inspect the Debian package:

  ```sh
  ./scripts/package-deb.sh
  ./scripts/smoke-deb.sh
  ```

- On macOS Apple Silicon, build and inspect the disk image:

  ```sh
  ./scripts/package-macos-dmg.sh
  ./scripts/smoke-macos-dmg.sh
  ```

- Install the packages on clean test machines when possible.
- Check first-run system diagnostics.
- Test typing into a harmless target window.
- Test OCR from a copied image.

## Publish

- Commit the version and metadata updates.
- Tag the release as `vX.Y.Z`.
- Push the tag.
- Create a GitHub release from the tag.
- Confirm the release workflow uploads the `.deb` and `.dmg` artifacts.
- Download the published artifacts and run one final install smoke test.
