#!/usr/bin/env bash
# Build the plugin and lay it out as a SolidCraft plugin folder: dist/attractor-pattern/.
# Copy that folder into SolidCraft's {data}/plugins/ directory.
set -euo pipefail
cd "$(dirname "$0")"
rustup target add wasm32-wasip2 >/dev/null
cargo build --release --target wasm32-wasip2
out=dist/attractor-pattern
rm -rf "$out"
mkdir -p "$out"
cp plugin.json "$out/"
cp target/wasm32-wasip2/release/attractor_pattern.wasm "$out/"
echo "built $out ($(wc -c < "$out/attractor_pattern.wasm") bytes)"
