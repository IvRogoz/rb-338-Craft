$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    $bindgen = Join-Path $PSScriptRoot 'tools/bin/wasm-bindgen.exe'
    if (-not (Test-Path -LiteralPath $bindgen)) {
        cargo install wasm-bindgen-cli --version 0.2.130 --locked --root tools
        if ($LASTEXITCODE -ne 0) { throw 'wasm-bindgen installation failed' }
    }
    rustup target add wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw 'Wasm target installation failed' }
    cargo build --release --target wasm32-unknown-unknown -p rb-engine -p rb-web
    if ($LASTEXITCODE -ne 0) { throw 'Rust WebAssembly build failed' }
    New-Item -ItemType Directory -Path dist -Force | Out-Null
    Copy-Item -LiteralPath 'crates/web/static/index.html','crates/web/static/style.css','crates/web/static/audio.js','crates/web/static/processor.js','crates/web/static/midi.js' -Destination dist
    Copy-Item -LiteralPath 'target/wasm32-unknown-unknown/release/rb_engine.wasm' -Destination 'dist/engine.wasm'
    & $bindgen 'target/wasm32-unknown-unknown/release/rb_web.wasm' --target web --out-dir dist --out-name app --no-typescript
    if ($LASTEXITCODE -ne 0) { throw 'Browser bindings generation failed' }
    Write-Host 'Built dist/. Run: cargo run -p rb-devserver'
} finally { Pop-Location }
