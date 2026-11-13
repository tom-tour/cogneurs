#!/usr/bin/env sh
# Builds the Rust engine to WebAssembly and copies it next to the page.
set -e
cd "$(dirname "$0")"
cargo build --release -p cogneurs-engine --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/cogneurs_engine.wasm web/engine.wasm
echo "web/engine.wasm ready"
