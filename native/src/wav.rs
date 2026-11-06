//! Offline, sample-free WAV export of the selected song loop.
//! ReBirth's manual p. 142 directs users to include extra bars for delay tails;
//! this exporter renders exactly the selected region and does not append tails.
use rb_engine::Engine;
use rb_model::Song;

const RATE: usize = 44_100;
const MAX_BYTES: usize = 256 * 1024 * 1024;
const BLOCK: usize = 128;

pub struct WavExport {
    engine: Engine,
    remaining: usize,
    output: Vec<u8>,
}
impl WavExport {
    pub fn new(mut song: Song) -> Result<Self, String> {
        song.validate()?;
        let end = song
            .start
            .checked_add(song.length)
            .ok_or("Loop range overflow")?;
        if end > song.arrangement.len() {
            return Err("Loop extends beyond the song arrangement".into());
        }
        let frames_f64 = (song.length as f64) * 4.0 * 60.0 / song.bpm as f64 * RATE as f64;
        if !frames_f64.is_finite() || frames_f64 < 1.0 {
            return Err("Invalid loop duration".into());
        }
        let frames = frames_f64.round() as usize;
        let data_bytes = frames.checked_mul(4).ok_or("Export size overflow")?;
        let total = data_bytes.checked_add(44).ok_or("Export size overflow")?;
        if total > MAX_BYTES || data_bytes > u32::MAX as usize - 36 {
            return Err("WAV export exceeds 256 MiB limit".into());
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(total)
            .map_err(|_| "Unable to allocate WAV export")?;
        output.extend_from_slice(b"RIFF");
        output.extend_from_slice(&(36 + data_bytes as u32).to_le_bytes());
        output.extend_from_slice(b"WAVEfmt ");
        output.extend_from_slice(&16u32.to_le_bytes());
        output.extend_from_slice(&1u16.to_le_bytes());
        output.extend_from_slice(&2u16.to_le_bytes());
        output.extend_from_slice(&(RATE as u32).to_le_bytes());
        output.extend_from_slice(&((RATE * 4) as u32).to_le_bytes());
        output.extend_from_slice(&4u16.to_le_bytes());
        output.extend_from_slice(&16u16.to_le_bytes());
        output.extend_from_slice(b"data");
        output.extend_from_slice(&(data_bytes as u32).to_le_bytes());
        let start = song.start;
        song.song_mode = true;
        song.looping = false;
        let mut engine = Engine::new(RATE as f32);
        engine.set_song(song);
        engine.command(2, start as u32);
        engine.command(1, 0);
        Ok(Self {
            engine,
            remaining: frames,
            output,
        })
    }
    /// Render at most `max_frames` and report whether the export is complete.
    /// A zero request makes no progress, allowing callers to cancel separately.
    pub fn render_chunk(&mut self, max_frames: usize) -> bool {
        let mut todo = self.remaining.min(max_frames);
        let mut buffer = [0f32; BLOCK * 2];
        while todo > 0 {
            let n = todo.min(BLOCK);
            self.engine.render(&mut buffer[..n * 2]);
            for &x in &buffer[..n * 2] {
                let sample = (x.clamp(-1.0, 1.0) * 32767.0).round() as i16;
                self.output.extend_from_slice(&sample.to_le_bytes());
            }
            self.remaining -= n;
            todo -= n;
        }
        self.remaining == 0
    }
    pub fn finish(self) -> Result<Vec<u8>, String> {
        if self.remaining != 0 {
            Err("WAV export is incomplete".into())
        } else {
            Ok(self.output)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn header_duration_and_nonzero_audio() {
        let mut s = Song::default();
        s.start = 1;
        s.length = 1;
        s.bpm = 500.;
        let mut e = WavExport::new(s).unwrap();
        assert!(!e.render_chunk(0));
        while !e.render_chunk(997) {}
        let wav = e.finish().unwrap();
        let frames = (4.0f64 * 60.0 / 500.0 * 44100.0).round() as usize;
        assert_eq!(wav.len(), 44 + frames * 4);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 44100);
        assert_eq!(
            u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize,
            frames * 4
        );
        assert!(wav[44..].iter().any(|x| *x != 0));
    }
    #[test]
    fn incomplete_and_oversize_rejected() {
        let mut s = Song::default();
        s.bpm = 20.;
        s.length = 4096;
        s.arrangement = vec![[0, 1, 2, 3]; 4096];
        assert!(WavExport::new(s).is_err());
        let e = WavExport::new(Song::default()).unwrap();
        assert!(e.finish().is_err());
    }
    #[test]
    fn muted_tracks_produce_zero_pcm() {
        let mut s = Song::default();
        s.length = 1;
        s.bpm = 500.;
        for c in &mut s.channels {
            c.mute = true;
        }
        let mut e = WavExport::new(s).unwrap();
        while !e.render_chunk(8192) {}
        let w = e.finish().unwrap();
        assert!(w[44..].iter().all(|x| *x == 0));
    }
}
