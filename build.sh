#!/usr/bin/env sh
set -eu

mkdir -p dist

cargo build --release
cp target/release/jumpbox-typer dist/jumpbox-typer-linux-amd64
mkdir -p dist/assets
cp assets/jumpbox-typer.svg dist/assets/jumpbox-typer.svg
