#!/usr/bin/env bash
# Build the plugin and package it for SolidCraft 0.41: dist/attractor-pattern-<version>.zip.
# Install the zip from Preferences > Plugins > Install plugin..., or with plugin.install.
#
# Needs the wasm32-unknown-unknown Rust target and wasm-tools (cargo install wasm-tools).
# SolidCraft links no WASI, so the module is built for wasm32-unknown-unknown and wrapped as a
# component without a WASI adapter.
set -euo pipefail
cd "$(dirname "$0")"
rustup target add wasm32-unknown-unknown >/dev/null
cargo build --release --target wasm32-unknown-unknown
version=$(python3 -c 'import json; print(json.load(open("plugin.json"))["version"])')
out=dist/attractor-pattern
rm -rf dist
mkdir -p "$out"
wasm-tools component new target/wasm32-unknown-unknown/release/attractor_pattern.wasm -o "$out/attractor-pattern.wasm"
wasm-tools validate "$out/attractor-pattern.wasm"
cp plugin.json "$out/"
(cd dist && zip -qr "attractor-pattern-$version.zip" attractor-pattern)
echo "built dist/attractor-pattern-$version.zip"
