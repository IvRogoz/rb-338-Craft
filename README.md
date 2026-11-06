# RB-338 Craft

A local, browser-based music workstation with the ReBirth RB-338 rack: two 303-style bass synthesizers, 808-style and 909-style drum machines, pattern sequencing, and effects.

Rust powers the interface, song model, sequencer, and audio engine. WebAssembly runs the engine inside an AudioWorklet, with small JavaScript adapters for browser audio and MIDI APIs. All sound is synthesized at runtime without recorded samples or waveform ROMs.

![RB-338 Craft showing the current rack, top Song / File launcher, synths, drum machines, mixers, and effects](app-screenshot.png)

## Features

- Two bass synths with saw/square oscillators, resonant filters, envelopes, accents, and tied slides.
- Synthesized 808-style and 909-style percussion, including shared 808 voice switches and 909 accents and flams.
- 32 patterns per section, independent pattern lengths and shuffle, and pattern copy, paste, randomize, and clear tools.
- Song arrangements, loop ranges, measure editing, and automation recording.
- Section mixers with level, pan, mute, delay sends, and insert routing.
- Distortion, a pattern-controlled filter (PCF), compression, and tempo-synchronized delay.
- Browser-local song saves, JSON import/export, and original ReBirth `.rbs` import.
- Stereo WAV loop export and MIDI input with optional clock/transport sync.

The rack scales proportionally to fit the browser window. Its panels, knobs, switches, keys, meters, and displays are rendered in HTML/CSS rather than interface images or sprites. **SONG / FILE · LIBRARY** at the top opens a modal for song files, arrangements, MIDI, and supplementary controls.

## Build and run locally

### Requirements

- A native Rust toolchain with `cargo` and `rustup` on your path.
- PowerShell on Windows, or a POSIX shell on macOS/Linux.
- A browser supporting WebAssembly, Web Audio, and AudioWorklet.
- Internet access for the initial tool and dependency downloads.

Run these commands from the repository root.

**Windows (PowerShell)**

```powershell
.\build.ps1
cargo run --release -p rb-devserver
```

**macOS / Linux**

```sh
sh build.sh
cargo run --release -p rb-devserver
```

Open [http://127.0.0.1:3380](http://127.0.0.1:3380) and press Play to initialize audio and start playback. Stop the server with `Ctrl+C`.

The build scripts add Rust's `wasm32-unknown-unknown` target, install `wasm-bindgen-cli` version `0.2.130` into the project-local `tools/` directory when absent, and generate the app in `dist/`. The application uses a native Rust build pipeline with no Node.js runtime, JavaScript package manager, or bundler.

The server binds only to `127.0.0.1`, serves `dist/`, and does not open a browser automatically. Once built, the app needs no external service. To change the port:

```sh
cargo run --release -p rb-devserver -- 3381
```

Then open [http://127.0.0.1:3381](http://127.0.0.1:3381).

## Using the rack

1. **Choose patterns.** Select a bank, then a pattern number in each section's left-hand pattern selector.
2. **Program synths.** Use the pitch buttons and note/pause, octave, accent, slide, back, and step controls. Pitch Mode advances after pitch entry.
3. **Program drums.** Select an instrument and edit its steps. The 808 shared-voice switches choose tom/conga, rim/claves, and clap/maracas; the 909 also supports accented hits and flams.
4. **Shape the mix.** Adjust section level, pan, and delay send. Insert processing runs distortion → PCF → compression. PCF and compression each select one destination; compression can also process the master.
5. **Arrange a song.** Open **SONG / FILE · LIBRARY** at the top to add scenes and edit measures. Set **Start** and **Length** in the transport and enable **Song mode** to play the arrangement.

Press **Space** to toggle playback. The keys `A W S E D F T G Y H U J` enter chromatic pitches for the selected synth. These shortcuts are disabled while the song modal is open or focus is in an input field or selector.

Delay follows the tempo with straight sixteenths or eighth-note triplets. Stop ends sequencing while retaining echo tails; a second Stop clears the previous tail while repositioning.

### Song / File modal

Open the top **SONG / FILE · LIBRARY** button. Close the modal with **CLOSE** or **Escape**.

| Control | Purpose |
| --- | --- |
| **SAVE / LOAD / DELETE** | Manage named songs in the current browser's local storage. |
| **DOWNLOAD JSON** | Download a portable song file. |
| **IMPORT** | Load a JSON song or an original ReBirth `.rbs` file. |
| **EXPORT LOOP WAV** | Render the selected arrangement loop as stereo, 16-bit PCM at 44.1 kHz. |
| **ENABLE MIDI** | Request browser MIDI access and select inputs and channel. |
| **MIDI SYNC** | Follow incoming MIDI clock and transport messages. |

The modal also contains pattern and measure utilities, scene selection, Vintage, Metronome, and Delay Wet controls.

Browser saves belong to the browser and origin, including the port. Download JSON files for portable backups. Legacy `rb338:` saves are supported. Example `.rbs` files are in [`songs/rbs-examples/`](songs/rbs-examples/); see the [example song notes](songs/rbs-examples/README.md).

WAV export renders exactly the selected loop and does not append delay tails. Include extra bars when needed. Exports are limited to 256 MiB.

MIDI requires browser Web MIDI support and device permission. Choose a device or **ALL INPUTS**, then select a channel. Notes, mapped controller messages, and optional clock/transport synchronization are supported.

## Development and checks

Run the workspace tests:

```sh
cargo test --workspace
```

Render 12 seconds of the default song through the native audio engine:

```sh
cargo run --release -p rb-engine --example render -- default target/verification/mix.wav 12
```

The renderer accepts a song path (`.rbs` or JSON), output WAV path, duration in seconds, and an optional isolated drum ID (`kit * 16 + instrument row`). Its output is stereo, 16-bit PCM at 48 kHz, with peak, RMS, and clipping reports. Generated WAVs are outputs, never synthesis sources.

For silent browser checks, use [http://127.0.0.1:3380/?muted=1](http://127.0.0.1:3380/?muted=1). The engine and meters continue running through a zero-gain output node.

Tests cover import validation, patterns and transport, arrangements and automation, synthesis behavior, effects, MIDI decoding, and WAV export. Passing tests do not establish perceptual equivalence to the original ReBirth application.

## Project structure

| Path | Contents |
| --- | --- |
| [`crates/model/`](crates/model/) | Typed song data, validation, arrangements, legacy JSON conversion, and RBS import. |
| [`crates/engine/`](crates/engine/) | Synths, percussion, sequencing, effects, and the worklet ABI. |
| [`crates/web/`](crates/web/) | Rust browser interface, WAV export, MIDI decoding, browser adapters, and CSS rack. |
| [`crates/devserver/`](crates/devserver/) | Native loopback-only static file server. |
| [`songs/rbs-examples/`](songs/rbs-examples/) | Original ReBirth example songs used by importer tests. |
| `dist/` | Generated browser application. |
| `tools/` | Project-local build tools. |

## Sound fidelity and credits

This is a Rust reimplementation, not the original ReBirth audio engine. Exact ReBirth timbre matching has not been established. The 909 metallic voices are synthesized replacements for the original sample-based sounds. The runtime uses digital oscillators, envelopes, noise, and filters rather than electrical component-network simulation.

Synthesis references, source credits, and retained license notices are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). The original interface artwork described there is retained only as local reference material; the app renders its interface in HTML/CSS.
