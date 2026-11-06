use rb_engine::Engine;
use std::{
    mem::size_of,
    ptr,
    sync::{mpsc, Arc, Mutex},
    thread::{self, JoinHandle},
};
use windows_sys::Win32::Media::Audio::*;

pub enum AudioMessage {
    Song(Box<rb_model::Song>),
    Command(u32, u32),
    Mute(bool),
    FinishRecording(mpsc::SyncSender<Snapshot>),
    Shutdown,
}
#[derive(Clone)]
pub struct Snapshot {
    pub position: usize,
    pub tick: usize,
    pub playing: bool,
    pub metrics: [f32; 11],
    pub controls: rb_model::ControlState,
    pub rendered_frames: u64,
    pub peak: f32,
    pub bpm: f32,
    pub error: Option<String>,
}
pub struct AudioBackend {
    tx: mpsc::Sender<AudioMessage>,
    snapshot: Arc<Mutex<Snapshot>>,
    thread: Option<JoinHandle<()>>,
}
impl AudioBackend {
    pub fn start(muted: bool) -> Result<Self, String> {
        let engine = Engine::new(48000.0);
        let snapshot = Arc::new(Mutex::new(Snapshot {
            position: 0,
            tick: 0,
            playing: false,
            metrics: [0.; 11],
            controls: engine.live_controls(),
            rendered_frames: 0,
            peak: 0.,
            bpm: 127.,
            error: None,
        }));
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let shared = snapshot.clone();
        let thread = thread::spawn(move || run(engine, muted, rx, shared, ready_tx));
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                tx,
                snapshot,
                thread: Some(thread),
            }),
            result => {
                let _ = thread.join();
                Err(match result {
                    Ok(Err(e)) => e,
                    _ => "Audio thread exited during startup".into(),
                })
            }
        }
    }
    pub fn send(&self, message: AudioMessage) {
        let _ = self.tx.send(message);
    }
    pub fn finish_recording(&self) -> Result<Snapshot, String> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.tx
            .send(AudioMessage::FinishRecording(tx))
            .map_err(|e| format!("Audio thread unavailable: {e}"))?;
        rx.recv_timeout(std::time::Duration::from_secs(2))
            .map_err(|e| format!("Recording acknowledgement failed: {e}"))
    }
    pub fn sender(&self) -> mpsc::Sender<AudioMessage> {
        self.tx.clone()
    }
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.lock().unwrap().clone()
    }
}
impl Drop for AudioBackend {
    fn drop(&mut self) {
        let _ = self.tx.send(AudioMessage::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

const FRAMES: usize = 512;
const BUFFERS: usize = 4;
#[repr(align(8))]
struct AlignedHeader(WAVEHDR);
struct Buffer {
    samples: Box<[i16; FRAMES * 2]>,
    header: Box<AlignedHeader>,
    queued: bool,
    prepared: bool,
}
fn run(
    mut engine: Engine,
    mut muted: bool,
    rx: mpsc::Receiver<AudioMessage>,
    snapshot: Arc<Mutex<Snapshot>>,
    ready: mpsc::SyncSender<Result<(), String>>,
) {
    unsafe {
        let format = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_PCM as u16,
            nChannels: 2,
            nSamplesPerSec: 48000,
            nAvgBytesPerSec: 192000,
            nBlockAlign: 4,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        let mut handle: HWAVEOUT = ptr::null_mut();
        let result = waveOutOpen(&mut handle, WAVE_MAPPER, &format, 0, 0, 0);
        if result != 0 {
            let _ = ready.send(Err(format!("waveOutOpen failed: {result}")));
            return;
        }
        let mut buffers: Vec<Buffer> = (0..BUFFERS)
            .map(|_| {
                let mut samples = Box::new([0i16; FRAMES * 2]);
                let header = Box::new(AlignedHeader(WAVEHDR {
                    lpData: samples.as_mut_ptr() as *mut u8,
                    dwBufferLength: (FRAMES * 4) as u32,
                    ..Default::default()
                }));
                Buffer {
                    samples,
                    header,
                    queued: false,
                    prepared: false,
                }
            })
            .collect();
        let mut error = None;
        for b in &mut buffers {
            let code = waveOutPrepareHeader(handle, &mut b.header.0, size_of::<WAVEHDR>() as u32);
            if code != 0 {
                error = Some(format!("waveOutPrepareHeader failed: {code}"));
                break;
            }
            b.prepared = true;
        }
        if error.is_none() {
            for b in &mut buffers {
                let code = waveOutWrite(handle, &mut b.header.0, size_of::<WAVEHDR>() as u32);
                if code != 0 {
                    error = Some(format!("waveOutWrite failed: {code}"));
                    break;
                }
                b.queued = true;
            }
        }
        if let Some(e) = error.take() {
            let cleanup_error = cleanup(handle, buffers);
            let e = match cleanup_error {
                Some(cleanup) => format!("{e}; {cleanup}"),
                None => e,
            };
            let _ = ready.send(Err(e));
            return;
        }
        let _ = ready.send(Ok(()));
        let mut running = true;
        let mut rendered = 0u64;
        let mut floats = [0f32; FRAMES * 2];
        while running {
            while let Ok(message) = rx.try_recv() {
                match message {
                    AudioMessage::Song(song) => engine.set_song(*song),
                    AudioMessage::Command(c, v) => engine.command(c, v),
                    AudioMessage::Mute(m) => muted = m,
                    AudioMessage::FinishRecording(reply) => {
                        engine.command(6, 0);
                        let mut s = snapshot.lock().unwrap();
                        s.position = engine.position() as usize;
                        s.tick = engine.tick() as usize;
                        s.playing = engine.playing();
                        s.metrics = engine.metrics();
                        s.controls = engine.live_controls();
                        s.bpm = engine.tempo();
                        s.rendered_frames = rendered;
                        let _ = reply.send(s.clone());
                    }
                    AudioMessage::Shutdown => running = false,
                }
            }
            if !running {
                break;
            }
            for b in &mut buffers {
                if b.queued && (ptr::addr_of!(b.header.0.dwFlags).read_volatile() & WHDR_DONE) == 0
                {
                    continue;
                }
                engine.render(&mut floats);
                if floats.iter().any(|sample| !sample.is_finite()) {
                    error = Some("Audio engine produced non-finite output".into());
                    running = false;
                    break;
                }
                let peak = floats.iter().fold(0f32, |a, &v| a.max(v.abs()));
                for (dst, &sample) in b.samples.iter_mut().zip(&floats) {
                    *dst = if muted { 0 } else { pcm(sample) };
                }
                let code = waveOutWrite(handle, &mut b.header.0, size_of::<WAVEHDR>() as u32);
                if code != 0 {
                    error = Some(format!("waveOutWrite failed: {code}"));
                    running = false;
                    break;
                }
                b.queued = true;
                rendered += FRAMES as u64;
                let mut s = snapshot.lock().unwrap();
                s.bpm = engine.tempo();
                s.position = engine.position() as usize;
                s.tick = engine.tick() as usize;
                s.playing = engine.playing();
                s.metrics = engine.metrics();
                s.controls = engine.live_controls();
                s.rendered_frames = rendered;
                s.peak = peak;
            }
            if running {
                thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        if let Some(cleanup_error) = cleanup(handle, buffers) {
            error = Some(match error {
                Some(e) => format!("{e}; {cleanup_error}"),
                None => cleanup_error,
            });
        }
        if let Some(e) = error {
            snapshot.lock().unwrap().error = Some(e);
        }
    }
}
// A failed unprepare leaves ownership with the driver, so retain that allocation.
unsafe fn cleanup(handle: HWAVEOUT, buffers: Vec<Buffer>) -> Option<String> {
    let mut error = None;
    let code = waveOutReset(handle);
    if code != 0 {
        error = Some(format!("waveOutReset failed: {code}"));
    }
    for mut b in buffers {
        if b.prepared {
            let code = waveOutUnprepareHeader(handle, &mut b.header.0, size_of::<WAVEHDR>() as u32);
            if code != 0 {
                if error.is_none() {
                    error = Some(format!("waveOutUnprepareHeader failed: {code}"));
                }
                std::mem::forget(b);
            }
        }
    }
    let code = waveOutClose(handle);
    if code != 0 && error.is_none() {
        error = Some(format!("waveOutClose failed: {code}"));
    }
    error
}
fn pcm(v: f32) -> i16 {
    (v.clamp(-1., 1.) * i16::MAX as f32) as i16
}
#[cfg(test)]
mod tests {
    #[test]
    fn pcm_limits() {
        assert_eq!(super::pcm(2.), 32767);
        assert_eq!(super::pcm(-2.), -32767);
        assert_eq!(super::pcm(0.), 0);
    }
}
