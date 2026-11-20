#!/bin/sh
# Builds the web demo into dist/. Needs: rustup target add wasm32-unknown-unknown,
# and wasm-bindgen-cli at the version in Cargo.lock.
set -e
cd "$(dirname "$0")/.."
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-dir dist target/wasm32-unknown-unknown/release/mailsort.wasm
cp web/index.html assets/style.css dist/
