#!/bin/sh
# Builds the web demo into dist/. Needs: rustup target add wasm32-unknown-unknown,
# and wasm-bindgen-cli at the version in Cargo.lock.
set -e
cd "$(dirname "$0")/.."
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-dir dist target/wasm32-unknown-unknown/release/mailsort.wasm
cp assets/style.css assets/logo-dark.svg dist/
# New URLs per build, so browsers don't keep a cached older demo.
build=$(cat dist/mailsort_bg.wasm dist/style.css | md5sum | cut -c1-8)
sed "s/BUILD/$build/g" web/index.html > dist/index.html
