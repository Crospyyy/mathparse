#!/usr/bin/env bash
set -eu

cd -- "$(dirname -- "${BASH_SOURCE[0]}")"

cargo build --locked --package quickmafs --release
strip -s target/release/quickmafs
cargo generate-rpm -p gui
