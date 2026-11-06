# RB-338 Native

A separate native Windows version of RB-338 Craft, built with Rust and eframe. Its interface uses native rendering, its audio engine runs directly in the executable, and it requires no browser, WebView, WebAssembly, Node.js, or local server.

This folder contains its own Rust workspace, a copy of the synthesis engine and song model, and importer fixtures. The web version in the parent workspace is separate.

## Download and run

Get `RB-338-Craft-v0.1.0-windows-x64.zip` from the [v0.1.0 release](https://github.com/IvRogoz/rb-338-Craft/releases/tag/v0.1.0), extract it, and double-click `RB-338-Craft.exe`. The ZIP includes this README, the third-party notices, and dependency/font license texts. `SHA256SUMS.txt` on the release page contains the archive's checksum.

This build targets 64-bit Windows. Native graphics and Windows audio drivers are required. It links to the Microsoft C++ runtime; install the [Microsoft Visual C++ v14 x64 Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/) if Windows reports a missing `VCRUNTIME140.dll`. The release is available only to people with access to the private repository.

## Controls and files

The rack provides two bass synths, 808-style and 909-style drums, four pattern selectors, section mixers, and the master effects. The native painter uses the web rack's 624×527 design coordinates, four equal-height sections, metallic surfaces, seven-segment counters, recessed switches, and piano programming buttons. It loads Arial from Windows when available and otherwise uses the bundled font fallback. It scales proportionally with the native window. Drag knobs vertically, use the wheel, or focus a knob and use the arrow keys. Space toggles playback; `A W S E D F T G Y H U J` enter synth pitches while the song modal is closed.

**SONG / FILE · LIBRARY** at the top opens the native song modal for import, JSON saving, the named library, scenes and measure editing, pattern utilities, MIDI, and WAV export. Close it with **CLOSE** or Escape. Import accepts ReBirth `.rbs`, current JSON, and legacy JSON; saving produces JSON. WAV export renders the selected arrangement loop as 44.1 kHz stereo, 16-bit PCM, up to 256 MiB, without appended delay tails.

Audio output uses WinMM at 48 kHz, stereo, 16-bit. Native Windows MIDI supports a selected input device and channel, controller/panel mappings, and external clock/transport sync. Named saves are in `%LOCALAPPDATA%\RB338Craft\Songs`.

## Build from source

Install Rust with the MSVC toolchain and the Microsoft C++ build tools. OpenGL-capable graphics are required for the interface. From `native/` in a source checkout, run in PowerShell:

```powershell
.\build.ps1
```

The executable is `native/target/release/rb338-native.exe` relative to the repository root. Double-click it to open the app; no server needs to be started. Native graphics and Windows audio drivers are required at runtime.

For noninteractive verification, run these commands from this directory:

```powershell
cargo test --workspace
cargo run --release -- --self-test
```

The executable's `--self-test` path creates no window and keeps device output muted. A functioning Windows audio output device is still required. It checks native audio-buffer submission and transport, recording/end acknowledgement, JSON round trips, RBS import, and WAV output. It also exercises the rack drawing code at two simulated window sizes and checks the generated meshes for invalid geometry; this is not a visual comparison. It writes `target/self-test/report.json`, `loop.wav`, and test song files relative to the current directory. The importer fixture is embedded in the executable, so this check does not require the original build location.

Native-window rendering, mouse/keyboard interaction, file dialogs, and physical MIDI input have not been verified because desktop interaction was disabled for this task. Automated checks do not establish those behaviors or perceptual equivalence to the original ReBirth application.

Synthesis references and retained licenses are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
