#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod controller;
mod midi_decode;
mod midi_device;
mod platform;
mod ui;
mod wav;

use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|arg| arg == "--self-test") {
        return self_test().map_err(Into::into);
    }
    let audio = audio::AudioBackend::start(false)?;
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("RB-338 Craft — Native Rust")
            .with_inner_size([1280.0, 1080.0])
            .with_min_inner_size([720.0, 640.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "RB-338 Craft Native",
        options,
        Box::new(move |cc| Ok(Box::new(ui::NativeApp::new(cc, audio)))),
    )?;
    Ok(())
}

fn self_test() -> Result<(), String> {
    let output = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join("target/self-test");
    std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let backend = audio::AudioBackend::start(true)?;
    let mut app = controller::Controller::new(backend);
    app.song.validate()?;
    app.action("play");
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut peak = 0.0f32;
    loop {
        std::thread::sleep(Duration::from_millis(20));
        app.poll();
        let snapshot = app.audio.snapshot();
        if let Some(error) = snapshot.error {
            return Err(error);
        }
        peak = peak.max(snapshot.peak);
        if snapshot.rendered_frames >= 12000 && app.tick > 0 && peak > 0.0001 {
            break;
        }
        if Instant::now() > deadline {
            return Err("Native audio/transport did not advance".into());
        }
    }
    app.action("stop");
    std::thread::sleep(Duration::from_millis(100));
    app.poll();
    if app.playing {
        return Err("Stop command did not reach the audio engine".into());
    }
    app.song.song_mode = true;
    app.sync();
    app.action("record");
    app.action("play");
    std::thread::sleep(Duration::from_millis(150));
    app.poll();
    app.song.synth[0].cutoff = 1234.0;
    app.changed("synth:0:cutoff", 1234.0);
    app.action("stop");
    if app.recording
        || !app
            .song
            .automation
            .iter()
            .any(|event| event.track == 1 && event.controller == 3)
    {
        return Err("Native automation recording/end acknowledgement failed".into());
    }
    let json = output.join("round-trip.json");
    app.save_path(&json)?;
    let expected = app.song.clone();
    app.load_path(&json)?;
    if app.song != expected {
        return Err("Native JSON round trip changed the song".into());
    }
    let rbs = output.join("import-fixture.rbs");
    std::fs::write(&rbs, include_bytes!("../songs/rbs-examples/Peff-3pm.rbs"))
        .map_err(|e| e.to_string())?;
    app.load_path(&rbs)?;
    if (app.song.bpm - 90.0).abs() > 0.01 {
        return Err("RBS import BPM mismatch".into());
    }
    app.song.start = 0;
    app.song.length = 1;
    app.export_wav(&output.join("loop.wav"))?;
    let wav = std::fs::read(output.join("loop.wav")).map_err(|e| e.to_string())?;
    if wav.len() <= 44 || &wav[..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return Err("Native WAV export is invalid".into());
    }
    if u32::from_le_bytes(wav[24..28].try_into().unwrap()) != 44100
        || u16::from_le_bytes(wav[22..24].try_into().unwrap()) != 2
        || u16::from_le_bytes(wav[34..36].try_into().unwrap()) != 16
        || !wav[44..].iter().any(|&byte| byte != 0)
    {
        return Err("Native WAV format or rendered audio is invalid".into());
    }
    let snapshot = app.audio.snapshot();
    ui::check_renderer(app.audio)?;
    let report = serde_json::json!({
        "status":"passed", "output_muted":true,
        "rendered_frames":snapshot.rendered_frames,"pre_mute_peak":peak,
        "json_round_trip":true,"rbs_import":true,"wav_bytes":wav.len(),
        "automation_recording_and_stop_ack":true,
        "drawing_path_without_window":true,
        "midi_inputs":midi_device::devices()?.len(),
        "native_window":"UNVERIFIED — desktop interaction disabled by user"
    });
    std::fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    println!("{report}");
    Ok(())
}
