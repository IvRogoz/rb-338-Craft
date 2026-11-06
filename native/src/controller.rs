use crate::audio::{AudioBackend, AudioMessage};
use rb_model::{ControlState, Pattern, Song};
use std::{collections::HashSet, path::Path};
pub struct Controller {
    pub song: Song,
    pub track: usize,
    pub bank: [usize; 4],
    pub edit: [usize; 2],
    pub voices: [usize; 2],
    pub pitch_mode: [bool; 2],
    pub flam: bool,
    pub recording: bool,
    pub metronome: bool,
    pub status: String,
    pub position: usize,
    pub tick: usize,
    pub playing: bool,
    pub live_controls: Option<ControlState>,
    pub midi_sync: bool,
    pub midi_channel: u8,
    pub midi_program: bool,
    pub audio: AudioBackend,
    clipboard: Option<(usize, Pattern)>,
    measure_clipboard: Option<rb_model::arrangement::MeasureClipboard>,
    punch: Option<Punch>,
}
impl Controller {
    pub fn new(audio: AudioBackend) -> Self {
        let song = Song::default();
        Self {
            bank: song.slots.map(|x| x / 8),
            song,
            track: 0,
            edit: [0; 2],
            voices: [1; 2],
            pitch_mode: [true; 2],
            flam: false,
            recording: false,
            metronome: false,
            status: String::new(),
            position: 0,
            tick: 0,
            playing: false,
            live_controls: None,
            midi_sync: false,
            midi_channel: 0,
            midi_program: false,
            audio,
            clipboard: None,
            measure_clipboard: None,
            punch: None,
        }
    }
    pub fn sync(&self) {
        self.audio
            .send(AudioMessage::Song(Box::new(self.song.clone())));
    }
    pub fn command(&self, command: u32, value: u32) {
        self.audio.send(AudioMessage::Command(command, value));
    }
    pub fn poll(&mut self) {
        let s = self.audio.snapshot();
        self.position = s.position;
        self.tick = s.tick;
        self.playing = s.playing;
        if self.midi_sync && self.playing {
            self.song.bpm = s.bpm;
        }
        if let Some(error) = s.error {
            self.status = error;
            self.playing = false;
        }
        self.live_controls = Some(s.controls);
        if let Some(p) = self.punch.as_mut() {
            if p.advance(&mut self.song, self.tick) {
                self.sync();
            }
        }
    }
    fn end_recording(&mut self) -> bool {
        if !self.recording && self.punch.is_none() {
            return true;
        }
        let snapshot = match self.audio.finish_recording() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.status = format!("Could not finish recording: {error}");
                return false;
            }
        };
        self.position = snapshot.position;
        self.tick = snapshot.tick;
        self.playing = snapshot.playing;
        self.live_controls = Some(snapshot.controls);
        if let Some(mut p) = self.punch.take() {
            p.advance(&mut self.song, self.tick);
        }
        self.recording = false;
        true
    }
    pub fn action(&mut self, action: &str) {
        perform_action(self, action);
    }
    pub fn changed(&mut self, key: &str, value: f32) {
        if key == "fx:delay_steps" {
            record_event(self, 5, 1, self.song.fx.delay_steps as u8);
        } else if key == "fx:filter_pattern" {
            record_event(self, 7, 4, self.song.fx.filter_pattern as u8);
        } else {
            record_control(self, key, value as f64);
        }
        if key == "drum:1:8:level" {
            self.song.drums[1].voices[9].level = value;
        }
        if key == "drum:1:9:level" {
            self.song.drums[1].voices[8].level = value;
        }
        self.sync();
    }
    pub fn install_song(&mut self, song: Song) -> bool {
        self.command(0, 0);
        if !self.end_recording() {
            return false;
        }
        self.song = song;
        self.playing = false;
        self.live_controls = None;
        self.position = self.song.start * 16;
        self.tick = self.song.start * 32;
        self.bank = self.song.slots.map(|x| x / 8);
        self.edit = [0; 2];
        self.sync();
        self.command(5, 0);
        true
    }
    pub fn load_path(&mut self, path: &Path) -> Result<(), String> {
        use std::io::Read;
        const MAX_FILE: u64 = 16 * 1024 * 1024;
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_FILE {
            return Err("Song file exceeds 16 MiB".into());
        }
        let mut data = Vec::new();
        file.take(MAX_FILE + 1)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        if data.len() as u64 > MAX_FILE {
            return Err("Song file exceeds 16 MiB".into());
        }
        let song = rb_model::rbs::import(&data).or_else(|_| {
            let text = std::str::from_utf8(&data).map_err(|e| e.to_string())?;
            Song::from_json(text).or_else(|_| rb_model::rbs::import_legacy(text))
        })?;
        song.validate()?;
        if !self.install_song(song) {
            return Err(self.status.clone());
        }
        Ok(())
    }
    pub fn save_path(&self, path: &Path) -> Result<(), String> {
        self.song.validate()?;
        let data = serde_json::to_vec_pretty(&self.song).map_err(|e| e.to_string())?;
        atomic_write(path, &data)
    }
    pub fn export_wav(&self, path: &Path) -> Result<(), String> {
        let mut wav = crate::wav::WavExport::new(self.song.clone())?;
        while !wav.render_chunk(44100) {}
        atomic_write(path, &wav.finish()?)
    }
    pub fn midi(&mut self, bytes: &[u8]) {
        use crate::midi_decode::MidiEvent as E;
        let Some(event) = crate::midi_decode::decode(bytes, self.midi_channel) else {
            return;
        };
        match event {
            E::Control(c) => {
                rb_model::rbs::apply_automation(&mut self.song, &c);
                record_event(self, c.track, c.controller, c.value);
                self.sync();
            }
            E::MasterLevel(v) => {
                self.song.fx.master = v;
                self.sync()
            }
            E::Shuffle(v) => {
                self.song.shuffle = v;
                self.sync()
            }
            E::InstrumentSelect { kit, value } => {
                self.voices[kit] = ((value as usize * 12) / 128).min(11)
            }
            E::NoteOn { note, .. } => {
                if let Some(a) = midi_panel(self, note) {
                    self.action(&a)
                }
            }
            E::Stop if self.midi_sync => {
                self.playing = false;
                self.end_recording();
            }
            _ => {}
        }
    }
}
fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or("Invalid file name")?
        .to_string_lossy();
    let mut last = None;
    for i in 0..100 {
        let tmp = parent.join(format!(".{name}.{}.{}.tmp", std::process::id(), i));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
        {
            Ok(mut f) => {
                let result = (|| {
                    f.write_all(data)?;
                    f.sync_all()?;
                    drop(f);
                    replace_file(&tmp, path)
                })();
                if result.is_err() {
                    let _ = std::fs::remove_file(tmp);
                }
                return result.map_err(|e| e.to_string());
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => return Err(e.to_string()),
        }
    }
    Err(last.unwrap().to_string())
}
#[cfg(windows)]
fn replace_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    extern "system" {
        fn MoveFileExW(a: *const u16, b: *const u16, flags: u32) -> i32;
    }
    let a: Vec<u16> = src.as_os_str().encode_wide().chain(Some(0)).collect();
    let b: Vec<u16> = dst.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe { MoveFileExW(a.as_ptr(), b.as_ptr(), 9) } == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
#[cfg(not(windows))]
fn replace_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::rename(src, dst)
}
fn random() -> f64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEED: AtomicU64 = AtomicU64::new(0);
    let mut x = SEED.load(Ordering::Relaxed);
    if x == 0 {
        x = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
            | 1;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    SEED.store(x, Ordering::Relaxed);
    (x >> 11) as f64 / (1u64 << 53) as f64
}
fn pitch_with_key(note: u8, key: u8) -> u8 {
    (note / 12).clamp(2, 4) * 12 + key.min(12)
}
fn toggle_octave(note: u8, up: bool) -> u8 {
    let pitch = note % 12;
    let octave = (note / 12).clamp(2, 4);
    let desired = if up { 4 } else { 2 };
    (if octave == desired { 3 } else { desired }) * 12 + pitch
}
struct Punch {
    old: Vec<rb_model::Automation>,
    fresh: Vec<rb_model::Automation>,
    touched: HashSet<(u8, u8)>,
    last: usize,
}
impl Punch {
    fn new(song: &Song, tick: usize) -> Self {
        Self {
            old: song.automation.clone(),
            fresh: Vec::new(),
            touched: HashSet::new(),
            last: tick,
        }
    }
    fn rebuild(&self, song: &mut Song) {
        song.automation = self.old.iter().chain(&self.fresh).cloned().collect();
        song.automation.sort_by_key(|e| e.tick);
    }
    fn event(&mut self, song: &mut Song, tick: usize, track: u8, controller: u8, value: u8) {
        self.touched.insert((track, controller));
        self.old.retain(|e| {
            !(e.tick as usize == tick && e.track == track && e.controller == controller)
        });
        self.fresh.retain(|e| {
            !(e.tick as usize == tick && e.track == track && e.controller == controller)
        });
        self.fresh.push(rb_model::Automation {
            tick: tick as u32,
            track,
            controller,
            value,
        });
        self.last = tick;
        self.rebuild(song);
    }
    fn advance(&mut self, song: &mut Song, tick: usize) -> bool {
        let before = self.old.len();
        let last = self.last;
        let start = song.start * 32;
        let end = song.start.saturating_add(song.length) * 32;
        let wrapped = song.looping && tick < last && tick >= start && last < end;
        self.old.retain(|e| {
            let x = e.tick as usize;
            let crossed = if wrapped {
                (x > last && x < end) || (x >= start && x <= tick)
            } else {
                tick >= last && x > last && x <= tick
            };
            !(crossed && self.touched.contains(&(e.track, e.controller)))
        });
        self.last = tick;
        if self.old.len() != before {
            self.rebuild(song);
            true
        } else {
            false
        }
    }
}
fn record_control(a: &mut Controller, key: &str, v: f64) {
    let p: Vec<&str> = key.split(':').collect();
    let n = |i: usize| p.get(i).and_then(|x| x.parse::<usize>().ok()).unwrap_or(0);
    let (track, controller, value) = match p[0] {
        "synth" => {
            let c = match p[2] {
                "tune" => 2,
                "cutoff" => 3,
                "resonance" => 4,
                "envelope" => 5,
                "decay" => 6,
                "accent" => 7,
                _ => return,
            };
            let x = match c {
                2 => (v + 12.) / 24.,
                3 => (v - 100.) / 5900.,
                4 => v,
                5 => v / 5000.,
                6 => (v - 0.05) / 0.75,
                _ => v,
            };
            (1 + n(1) as u8, c, (x * 127.).round() as u8)
        }
        "channel" => {
            let i = n(1);
            let c = match p[2] {
                "level" => 6,
                "pan" => 7,
                "delay" => 8,
                _ => return,
            };
            let x = if c == 7 { (v + 1.) / 2. } else { v };
            (0, (c + i * 6) as u8, (x * 127.).round() as u8)
        }
        "drum" => {
            let k = n(1);
            let id = n(2);
            let field = p[3];
            let names = [
                "level", "tune", "decay", "tone", "snappy", "attack", "variant",
            ];
            let Some(f) = names.iter().position(|x| *x == field) else {
                return;
            };
            let maps: [&[(usize, usize)]; 2] = [
                &[
                    (0, 0),
                    (1, 0),
                    (1, 3),
                    (1, 2),
                    (2, 0),
                    (2, 1),
                    (2, 4),
                    (3, 0),
                    (3, 1),
                    (3, 6),
                    (4, 0),
                    (4, 1),
                    (4, 6),
                    (5, 0),
                    (5, 1),
                    (5, 6),
                    (6, 0),
                    (6, 6),
                    (7, 0),
                    (7, 6),
                    (8, 0),
                    (9, 0),
                    (9, 3),
                    (9, 2),
                    (10, 0),
                    (10, 2),
                    (11, 0),
                ],
                &[
                    (0, 0),
                    (1, 0),
                    (1, 1),
                    (1, 5),
                    (1, 2),
                    (2, 0),
                    (2, 1),
                    (2, 3),
                    (2, 4),
                    (3, 0),
                    (3, 1),
                    (3, 2),
                    (4, 0),
                    (4, 1),
                    (4, 2),
                    (5, 0),
                    (5, 1),
                    (5, 2),
                    (6, 0),
                    (7, 0),
                    (8, 0),
                    (8, 2),
                    (9, 2),
                    (10, 0),
                    (10, 1),
                    (11, 0),
                    (11, 1),
                ],
            ];
            let Some(c) = maps[k.min(1)].iter().position(|x| *x == (id, f)) else {
                return;
            };
            let x = match f {
                1 => (v + 6.) / 12.,
                2 => (v - 0.3) / 1.7,
                _ => v,
            };
            (3 + k as u8, (c + 2) as u8, (x * 127.).round() as u8)
        }
        "kit" if p[2] == "flam" => (4, 29, (((v - 0.005) / 0.095) * 127.).round() as u8),
        "fx" => {
            let (t, c, x) = match p[1] {
                "delay_feedback" => (5, 3, v),
                "delay_pan" => (5, 4, (v + 1.) / 2.),
                "distortion" => (6, 1, v),
                "distortion_shape" => (6, 2, v),
                "filter_cutoff" => (7, 1, (v - 100.) / 7900.),
                "filter_q" => (7, 2, (v - 0.5) / 14.5),
                "filter_amount" => (7, 3, v),
                "filter_decay" => (7, 5, (v - 0.1) / 9.9),
                "compressor_ratio" => (8, 1, (v - 1.) / 19.),
                "compressor_threshold" => (8, 2, 10f64.powf(v / 20.)),
                _ => return,
            };
            (t, c, (x * 127.).round() as u8)
        }
        _ => return,
    };
    record_event(a, track, controller, value.min(127));
}
fn record_event(a: &mut Controller, track: u8, controller: u8, value: u8) {
    if a.song.song_mode {
        a.command(7, (track as u32) << 8 | controller as u32);
    }
    if !(a.recording && a.song.song_mode) {
        return;
    }
    if let Some(p) = a.punch.as_mut() {
        p.advance(&mut a.song, a.tick);
        p.event(&mut a.song, a.tick, track, controller, value)
    }
}
fn toggle_state(song: &Song, playing: bool, live: Option<ControlState>) -> ControlState {
    if playing {
        live.unwrap_or_else(|| song.control_state())
    } else {
        song.control_state()
    }
}
fn control_action(song: &mut Song, live: &mut ControlState, parts: &[&str]) {
    let idx = |n: usize| {
        parts
            .get(n)
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0)
    };
    match parts[0] {
        "enable" => {
            let t = idx(1).min(3);
            live.enabled[t] = !live.enabled[t];
            song.enabled[t] = live.enabled[t];
        }
        "wave" => {
            let t = idx(1).min(1);
            live.synth[t].wave = if live.synth[t].wave == "saw" {
                "square"
            } else {
                "saw"
            }
            .into();
            song.synth[t].wave = live.synth[t].wave.clone();
        }
        "variant" => {
            let k = idx(1).min(1);
            let v = idx(2).min(11);
            live.drums[k].voices[v].variant = !live.drums[k].voices[v].variant;
            song.drums[k].voices[v].variant = live.drums[k].voices[v].variant;
        }
        "dist" => {
            let t = idx(1).min(3);
            live.channels[t].distortion = !live.channels[t].distortion;
            song.channels[t].distortion = live.channels[t].distortion;
        }
        "filter" => {
            let t = idx(1).min(3);
            let on = !live.channels[t].filter;
            for i in 0..4 {
                live.channels[i].filter = on && i == t;
                song.channels[i].filter = live.channels[i].filter;
            }
        }
        "comp" | "master_comp" => {
            let t = idx(1).min(3);
            let master = parts[0] == "master_comp";
            let on = if master {
                !live.fx.compressor_master
            } else {
                !live.channels[t].compressor
            };
            for i in 0..4 {
                live.channels[i].compressor = !master && on && i == t;
                song.channels[i].compressor = live.channels[i].compressor;
            }
            live.fx.compressor_master = master && on;
            song.fx.compressor_master = live.fx.compressor_master;
        }
        "fx_toggle" => {
            let (target, value) = match parts[1] {
                "delay_enabled" => (&mut song.fx.delay_enabled, &mut live.fx.delay_enabled),
                "delay_triplet" => (&mut song.fx.delay_triplet, &mut live.fx.delay_triplet),
                "filter_enabled" => (&mut song.fx.filter_enabled, &mut live.fx.filter_enabled),
                "distortion_enabled" => (
                    &mut song.fx.distortion_enabled,
                    &mut live.fx.distortion_enabled,
                ),
                "compressor_enabled" => (
                    &mut song.fx.compressor_enabled,
                    &mut live.fx.compressor_enabled,
                ),
                _ => return,
            };
            *value = !*value;
            *target = *value;
        }
        "filter_mode" => {
            live.fx.filter_mode = if live.fx.filter_mode == "LP" {
                "BP"
            } else {
                "LP"
            }
            .into();
            song.fx.filter_mode = live.fx.filter_mode.clone();
        }
        "fx_step" => {
            let d = parts[2].parse::<isize>().unwrap_or(0);
            if parts[1] == "delay_steps" {
                live.fx.delay_steps = (live.fx.delay_steps as isize + d).clamp(1, 32) as usize;
                song.fx.delay_steps = live.fx.delay_steps;
            } else {
                live.fx.filter_pattern =
                    (live.fx.filter_pattern as isize + d).clamp(0, 54) as usize;
                song.fx.filter_pattern = live.fx.filter_pattern;
            }
        }
        _ => {}
    }
}
fn perform_action(a: &mut Controller, action: &str) -> bool {
    let parts: Vec<&str> = action.split(':').collect();
    let idx = |n: usize| {
        parts
            .get(n)
            .and_then(|x| x.parse::<usize>().ok())
            .unwrap_or(0)
    };
    let t = if ["copy", "paste", "clear", "random"].contains(&parts[0]) {
        idx(1).min(3)
    } else {
        a.track
    };
    let slot = a.song.slots[t];
    match parts[0] {
        "track" => a.track = idx(1).min(3),
        "voice" => {
            let kit = idx(1).min(1);
            let voice = idx(2).min(11);
            a.voices[kit] = voice;
            if voice > 0 {
                a.command(3, (kit * 16 + voice) as u32);
            }
        }
        "bank" => a.bank[idx(1).min(3)] = idx(2).min(3),
        "pattern" => {
            let t = idx(1).min(3);
            a.song.slots[t] = a.bank[t] * 8 + idx(2).min(7);
            if a.recording && a.song.song_mode {
                let bar = (a.position / 16).min(4095);
                while a.song.arrangement.len() <= bar {
                    a.song.arrangement.push(a.song.slots);
                }
                a.song.arrangement[bar][t] = a.song.slots[t];
            }
        }
        "enable" | "wave" | "variant" | "dist" | "filter" | "comp" | "master_comp"
        | "fx_toggle" | "filter_mode" | "fx_step" => {
            let mut live = toggle_state(&a.song, a.playing, a.live_controls.take());
            control_action(&mut a.song, &mut live, &parts);
            a.live_controls = Some(live);
        }
        "shuffle_track" => {
            let t = idx(1).min(3);
            a.song.shuffle_tracks[t] = !a.song.shuffle_tracks[t]
        }
        "steps_down" | "steps_up" => {
            let t = idx(1).min(3);
            let n = &mut a.song.patterns[a.song.slots[t]].lengths[t];
            if parts[0] == "steps_up" {
                *n = (*n + 1).min(16)
            } else {
                *n = n.saturating_sub(1).max(1)
            }
        }
        "pitchmode" => {
            let t = idx(1).min(1);
            a.pitch_mode[t] = !a.pitch_mode[t];
            if a.pitch_mode[t] {
                a.edit[t] = 0
            }
        }
        "pitchkey" => {
            let t = idx(1).min(1);
            a.track = t;
            let n = &mut a.song.patterns[a.song.slots[t]].notes[t][a.edit[t]];
            n.note = pitch_with_key(n.note, idx(2).min(12) as u8);
            n.gate = true;
            if a.pitch_mode[t] {
                a.edit[t] = (a.edit[t] + 1) % 16
            }
        }
        "down" | "up" | "note" | "accent" | "slide" => {
            let t = idx(1).min(1);
            let n = &mut a.song.patterns[a.song.slots[t]].notes[t][a.edit[t]];
            match parts[0] {
                "down" => {
                    n.note = toggle_octave(n.note, false);
                }
                "up" => {
                    n.note = toggle_octave(n.note, true);
                }
                "note" => n.gate = !n.gate,
                "accent" => n.accent = !n.accent,
                _ => n.slide = !n.slide,
            }
        }
        "back" | "next" => {
            let t = idx(1).min(1);
            a.edit[t] = (a.edit[t] + if parts[0] == "back" { 15 } else { 1 }) % 16
        }
        "flam" => a.flam = !a.flam,
        "mute" => {
            let t = idx(1).min(3);
            a.song.channels[t].mute = !a.song.channels[t].mute
        }
        "midi_sync" => {
            a.midi_sync = !a.midi_sync;
            a.command(9, u32::from(a.midi_sync));
        }
        "song_mode" => a.song.song_mode = !a.song.song_mode,
        "pattern_mode" => a.song.song_mode = false,
        "loop" => a.song.looping = !a.song.looping,
        "vintage" => a.song.vintage = !a.song.vintage,
        "record" => {
            if a.recording {
                a.end_recording();
            } else {
                a.recording = true;
                a.command(6, 1);
                if a.song.automation_base.is_none() {
                    a.song.automation_base = Some(a.song.control_state());
                }
                a.punch = Some(Punch::new(&a.song, a.tick));
            }
        }
        "tempo" => {
            let d = parts[1].parse::<f32>().unwrap_or(0.);
            a.song.bpm = (a.song.bpm + d).clamp(20., 500.)
        }
        "transport" => {
            let d = parts[1].parse::<isize>().unwrap_or(0);
            a.position = ((a.position / 16) as isize + d)
                .max(0)
                .min(a.song.arrangement.len().saturating_sub(1) as isize)
                as usize
                * 16;
            a.tick = a.position * 2;
            if let Some(p) = a.punch.as_mut() {
                p.last = a.tick;
            }
            a.command(2, (a.position / 16) as u32)
        }
        "start" | "length" => {
            let d = parts[1].parse::<isize>().unwrap_or(0);
            if parts[0] == "start" {
                a.song.start = (a.song.start as isize + d)
                    .max(0)
                    .min(a.song.arrangement.len().saturating_sub(1) as isize)
                    as usize
            } else {
                a.song.length = (a.song.length as isize + d).clamp(1, 4096) as usize
            }
        }
        "scene" => {
            let i = idx(1);
            if let Some(slots) = a.song.arrangement.get(i) {
                a.song.slots = *slots;
                a.position = i * 16;
                a.tick = a.position * 2;
                if let Some(p) = a.punch.as_mut() {
                    p.last = a.tick;
                }
                a.command(2, i as u32)
            }
        }

        "drum" => {
            let k = idx(1).min(1);
            let v = idx(2).min(11);
            let n = &mut a.song.patterns[a.song.slots[k + 2]].drums[k][v][idx(3).min(15)];
            *n = if k == 1 && a.flam {
                if *n == 3 {
                    0
                } else {
                    3
                }
            } else if k == 1 {
                (*n + 1) % 3
            } else {
                u8::from(*n == 0)
            };
        }
        "copy" => {
            a.clipboard = Some((t, a.song.patterns[slot].clone()));
            a.status = format!("Pattern copied.");
            return false;
        }
        "paste" => {
            if let Some((source, p)) = &a.clipboard {
                if t < 2 && *source < 2 {
                    a.song.patterns[slot].notes[t] = p.notes[*source].clone()
                } else if t >= 2 && *source >= 2 {
                    a.song.patterns[slot].drums[t - 2] = p.drums[*source - 2].clone()
                } else {
                    a.status = format!("Copy between matching track types.");
                    return false;
                }
                a.song.patterns[slot].lengths[t] = p.lengths[*source];
            }
        }
        "clear" => {
            if t < 2 {
                for n in &mut a.song.patterns[slot].notes[t] {
                    n.note = 36;
                    n.gate = false;
                    n.accent = false;
                    n.slide = false
                }
            } else {
                for row in &mut a.song.patterns[slot].drums[t - 2] {
                    row.fill(0)
                }
            }
        }
        "random" => {
            if t < 2 {
                for n in &mut a.song.patterns[slot].notes[t] {
                    n.gate = random() < 0.7;
                    n.note = 36 + (random() * 24.) as u8;
                    n.accent = random() < 0.25;
                    n.slide = random() < 0.15
                }
            } else {
                for row in &mut a.song.patterns[slot].drums[t - 2] {
                    for n in row {
                        *n = if random() < 0.2 {
                            1 + (random() * 3.) as u8
                        } else {
                            0
                        }
                    }
                }
            }
        }
        "measure_copy" | "measure_cut" | "measure_insert" | "measure_replace" => {
            if a.recording {
                a.status = format!("Stop recording before editing measures.");
                return false;
            }
            let result = match parts[0] {
                "measure_copy" => a
                    .song
                    .copy_measures(a.song.start..a.song.start.saturating_add(a.song.length))
                    .map(|clip| a.measure_clipboard = Some(clip)),
                "measure_cut" => a
                    .song
                    .cut_measures(a.song.start..a.song.start.saturating_add(a.song.length))
                    .map(|clip| a.measure_clipboard = Some(clip)),
                _ => match &a.measure_clipboard {
                    Some(clip) => {
                        a.song
                            .paste_measures(a.position / 16, clip, parts[0] == "measure_replace")
                    }
                    None => Err("Copy or cut a loop first.".into()),
                },
            };
            if let Err(e) = result {
                a.status = e;
                return false;
            }
        }
        "scene_add" => {
            if a.song.arrangement.len() < 4096 {
                a.song.arrangement.push(a.song.slots)
            }
        }
        "scene_remove" => {
            a.song.arrangement.pop();
            a.song.start = a.song.start.min(a.song.arrangement.len().saturating_sub(1));
        }
        "scene_clear" => {
            a.song.arrangement.clear();
            a.song.start = 0
        }
        "stop" => {
            a.command(0, 0);
            a.playing = false;
            a.end_recording();
        }
        "metronome" => {
            a.metronome = !a.metronome;
            a.command(4, u32::from(a.metronome));
            a.status = if a.metronome {
                "Metronome on"
            } else {
                "Metronome off"
            }
            .into();
            return false;
        }
        "new" => {
            a.install_song(Song::default());
        }
        "play" => {
            if a.playing {
                a.command(0, 0);
                a.playing = false;
                a.end_recording();
            } else {
                a.command(1, 0);
                a.playing = true;
            }
        }
        _ => return false,
    }
    match parts[0] {
        "enable" => {
            let t = idx(1).min(3);
            record_event(a, (t + 1) as u8, 0, if a.song.enabled[t] { 127 } else { 0 });
        }
        "wave" => {
            let t = idx(1).min(1);
            record_event(
                a,
                (t + 1) as u8,
                8,
                if a.song.synth[t].wave == "square" {
                    127
                } else {
                    0
                },
            );
        }
        "variant" => {
            let k = idx(1).min(1);
            let v = idx(2).min(11);
            let value = if a.song.drums[k].voices[v].variant {
                1.
            } else {
                0.
            };
            record_control(a, &format!("drum:{k}:{v}:variant"), value);
        }
        "dist" => {
            let t = idx(1).min(3);
            record_event(
                a,
                0,
                (9 + 6 * t) as u8,
                if a.song.channels[t].distortion {
                    127
                } else {
                    0
                },
            );
        }
        "filter" => {
            let t = idx(1).min(3);
            record_event(
                a,
                0,
                2,
                if a.song.channels[t].filter {
                    (t + 2) as u8
                } else {
                    0
                },
            );
        }
        "comp" => {
            let t = idx(1).min(3);
            record_event(
                a,
                0,
                1,
                if a.song.channels[t].compressor {
                    (t + 2) as u8
                } else {
                    0
                },
            );
        }
        "master_comp" => record_event(a, 0, 1, if a.song.fx.compressor_master { 1 } else { 0 }),
        "fx_toggle" => {
            let (t, c, on) = match parts[1] {
                "delay_enabled" => (5, 0, a.song.fx.delay_enabled),
                "delay_triplet" => (5, 2, a.song.fx.delay_triplet),
                "filter_enabled" => (7, 0, a.song.fx.filter_enabled),
                "distortion_enabled" => (6, 0, a.song.fx.distortion_enabled),
                "compressor_enabled" => (8, 0, a.song.fx.compressor_enabled),
                _ => (0, 0, false),
            };
            record_event(a, t, c, if on { 127 } else { 0 });
        }
        "filter_mode" => record_event(
            a,
            7,
            6,
            if a.song.fx.filter_mode == "LP" {
                127
            } else {
                0
            },
        ),
        "fx_step" => {
            if parts[1] == "delay_steps" {
                record_event(a, 5, 1, a.song.fx.delay_steps as u8)
            } else {
                record_event(a, 7, 4, a.song.fx.filter_pattern as u8)
            }
        }
        _ => {}
    }
    a.sync();
    true
}
fn midi_panel(a: &mut Controller, note: u8) -> Option<String> {
    use crate::midi_decode::{KeyAction as K, PatternKey as P, SynthKey as S};
    if let Some(key) = crate::midi_decode::panel_key(note) {
        return match key {
            K::Focus(t) => {
                a.track = t;
                None
            }
            K::Play => Some("play".into()),
            K::Stop => Some("stop".into()),
            K::Record => Some("record".into()),
            K::BarBack => Some("transport:-1".into()),
            K::BarForward => Some("transport:1".into()),
            K::ToggleEffect(i) => Some(format!(
                "fx_toggle:{}",
                [
                    "filter_enabled",
                    "delay_enabled",
                    "distortion_enabled",
                    "compressor_enabled"
                ][i as usize]
            )),
            K::ToggleMix(t) => Some(format!("mute:{t}")),
            K::ToggleDist(t) => Some(format!("dist:{t}")),
            K::ToggleFilter(t) => Some(format!("filter:{t}")),
            K::ToggleCompressor(t) => Some(format!("comp:{t}")),
            K::ToggleMasterCompressor => Some("master_comp".into()),
            K::SelectPatternMode => {
                a.midi_program = false;
                None
            }
            K::ProgramSynthMode => {
                a.midi_program = true;
                None
            }
            K::SelectOrProgram => {
                a.midi_program = !a.midi_program;
                None
            }
        };
    }
    if a.midi_program {
        let t = a.track.min(1);
        return crate::midi_decode::synth_key(note).map(|k| match k {
            S::Pitch(n) => format!("pitchkey:{t}:{n}"),
            S::OctaveDown => format!("down:{t}"),
            S::OctaveUp => format!("up:{t}"),
            S::Accent => format!("accent:{t}"),
            S::Slide => format!("slide:{t}"),
            S::NotePause => format!("note:{t}"),
            S::Back => format!("back:{t}"),
            S::Step => format!("next:{t}"),
            S::PitchMode => format!("pitchmode:{t}"),
        });
    }
    crate::midi_decode::pattern_key(note).map(|k| match k {
        P::Toggle(t) => format!("enable:{t}"),
        P::Bank(t, b) => format!("bank:{t}:{b}"),
        P::Pattern(t, p) => format!("pattern:{t}:{p}"),
    })
}
#[cfg(test)]
mod controller_tests {
    use super::*;
    #[test]
    fn stopped_toggle_uses_displayed_song_value() {
        let mut song = Song::default();
        song.enabled[0] = true;
        let mut stale = song.control_state();
        stale.enabled[0] = false;
        let mut state = toggle_state(&song, false, Some(stale));
        control_action(&mut song, &mut state, &["enable", "0"]);
        assert!(!song.enabled[0]);
    }
    #[test]
    fn punch_replaces_touched_controls_and_preserves_others() {
        let mut song = Song::default();
        song.automation = vec![
            rb_model::Automation {
                tick: 2,
                track: 1,
                controller: 2,
                value: 10,
            },
            rb_model::Automation {
                tick: 3,
                track: 1,
                controller: 2,
                value: 11,
            },
            rb_model::Automation {
                tick: 3,
                track: 1,
                controller: 3,
                value: 12,
            },
        ];
        let mut punch = Punch::new(&song, 2);
        punch.event(&mut song, 2, 1, 2, 100);
        punch.event(&mut song, 2, 1, 2, 101);
        punch.advance(&mut song, 3);
        assert_eq!(song.automation.len(), 2);
        assert_eq!(song.automation[0].value, 101);
        assert_eq!(song.automation[1].controller, 3);
    }
    #[test]
    fn atomic_file_replacement() {
        let path =
            std::env::temp_dir().join(format!("rb338-controller-test-{}", std::process::id()));
        std::fs::write(&path, b"old").unwrap();
        atomic_write(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        std::fs::remove_file(path).unwrap();
    }
}
