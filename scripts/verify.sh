#!/usr/bin/env sh
set -eu

cargo fmt --all -- --check
cargo test --all-targets
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
./tests/verification.sh
./tests/packaging.sh
./build.sh
