//! Render the actual Rust engine to PCM for non-browser verification.
use rb_engine::Engine;
use rb_model::Song;
use std::{
    fs::File,
    io::{Seek, SeekFrom, Write},
    path::Path,
    time::Instant,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let input = args.get(1).map(String::as_str).unwrap_or("default");
    let output = args
        .get(2)
        .map(String::as_str)
        .unwrap_or("target/verification/mix.wav");
    let seconds: f32 = args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(12.);
    if !seconds.is_finite() || seconds <= 0. || seconds > 3600. {
        return Err("duration must be 0..3600 seconds".into());
    }
    let mut song = if input == "default" {
        Song::default()
    } else {
        let bytes = std::fs::read(input)?;
        if input.to_ascii_lowercase().ends_with(".rbs") {
            rb_model::rbs::import(&bytes)?
        } else {
            Song::from_json(std::str::from_utf8(&bytes)?)?
        }
    };
    // An optional kit/voice isolates an individual synthesized instrument.
    let voice = args.get(4).map(|s| s.parse::<u32>()).transpose()?;
    if voice.is_some() {
        song.fx.delay_enabled = false;
        song.fx.compressor_enabled = false;
    }
    if let Some(parent) = Path::new(output).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let sr = 48000u32;
    let frames = (seconds * sr as f32) as usize;
    let mut file = File::create(output)?;
    file.write_all(&[0u8; 44])?;
    let mut engine = Engine::new(sr as f32);
    engine.set_song(song);
    if let Some(id) = voice {
        engine.command(3, id);
    } else {
        engine.command(1, 0);
    }
    let started = Instant::now();
    let mut buffer = [0f32; 256];
    let mut peak = 0f32;
    let mut energy = 0f64;
    let mut clipped = 0usize;
    for offset in (0..frames).step_by(128) {
        let count = (frames - offset).min(128);
        engine.render(&mut buffer[..count * 2]);
        for &x in &buffer[..count * 2] {
            if !x.is_finite() {
                return Err("Non-finite output".into());
            }
            peak = peak.max(x.abs());
            energy += (x as f64).powi(2);
            clipped += usize::from(x.abs() >= 1.);
            file.write_all(&((x * 32767.) as i16).to_le_bytes())?;
        }
    }
    let bytes = (frames * 4) as u32;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(b"RIFF")?;
    file.write_all(&(bytes + 36).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&2u16.to_le_bytes())?;
    file.write_all(&sr.to_le_bytes())?;
    file.write_all(&(sr * 4).to_le_bytes())?;
    file.write_all(&4u16.to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&bytes.to_le_bytes())?;
    println!(
        "{output}: {seconds}s, peak={peak:.6}, RMS={:.6}, clipped={clipped}, render={:?}",
        (energy / (frames * 2) as f64).sqrt(),
        started.elapsed()
    );
    Ok(())
}
