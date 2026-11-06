#!/bin/sh
set -eu
cd "$(dirname "$0")"
if [ ! -x tools/bin/wasm-bindgen ]; then
  cargo install wasm-bindgen-cli --version 0.2.130 --locked --root tools
fi
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown -p rb-engine -p rb-web
mkdir -p dist
cp crates/web/static/index.html crates/web/static/style.css crates/web/static/audio.js crates/web/static/processor.js crates/web/static/midi.js dist/
cp target/wasm32-unknown-unknown/release/rb_engine.wasm dist/engine.wasm
tools/bin/wasm-bindgen target/wasm32-unknown-unknown/release/rb_web.wasm --target web --out-dir dist --out-name app --no-typescript
printf 'Built dist/. Run: cargo run -p rb-devserver\n'
