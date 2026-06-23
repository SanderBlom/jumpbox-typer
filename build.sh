#!/usr/bin/env sh
set -eu

mkdir -p dist

cargo build --release
cp target/release/jumpbox-typer dist/jumpbox-typer-linux-amd64
