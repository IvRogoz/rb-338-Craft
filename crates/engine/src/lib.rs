//! Sample-free Rust synthesis and musical sequencing, without circuit simulation.
use rb_model::{Automation, Song};
use std::collections::HashSet;
mod acid;
mod drums;
mod fx;
mod patterns;
use acid::AcidVoice;
use drums::DrumSynth;

pub struct Engine {
    song: Song,
    current: Song,
    sr: f32,
    playing: bool,
    recording: bool,
    midi_sync: bool,
    midi_elapsed: f64,
    midi_seen: bool,
    bar: usize,
    half: usize,
    step: usize,
    time: f64,
    next_half: f64,
    track_time: [f64; 4],
    track_step: [usize; 4],
    track_count: [usize; 4],
    active_slots: [usize; 4],
    auto_cursor: usize,
    manual: HashSet<(u8, u8)>,
    manual_paths: HashSet<String>,
    controls_revision: u32,
    voices: [AcidVoice; 2],
    drums: [DrumSynth; 2],
    flams: [[[f32; 16]; 12]; 2],
    flam_gain: [[[f32; 16]; 12]; 2],
    rack: fx::Rack,
    metronome: bool,
    click: f32,
    click_phase: f32,
}
impl Engine {
    pub fn new(sr: f32) -> Self {
        let sr = if sr.is_finite() {
            sr.clamp(8000., 192000.)
        } else {
            48000.
        };
        let song = Song::default();
        Self {
            current: song.clone(),
            active_slots: song.slots,
            song,
            sr,
            playing: false,
            recording: false,
            midi_sync: false,
            midi_elapsed: 0.,
            midi_seen: false,
            bar: 0,
            half: 0,
            step: 0,
            time: 0.,
            next_half: 0.,
            track_time: [0.; 4],
            track_step: [0; 4],
            track_count: [0; 4],
            auto_cursor: 0,
            manual: HashSet::new(),
            manual_paths: HashSet::new(),
            controls_revision: 1,
            voices: std::array::from_fn(|_| AcidVoice::new(sr)),
            drums: std::array::from_fn(|k| DrumSynth::new(sr, k)),
            flams: [[[0.; 16]; 12]; 2],
            flam_gain: [[[0.; 16]; 12]; 2],
            rack: fx::Rack::new(sr),
            metronome: false,
            click: 0.,
            click_phase: 0.,
        }
    }
    pub fn set_song(&mut self, song: Song) {
        if self.playing && !song.automation.is_empty() {
            let mut edited = HashSet::new();
            different_paths(
                &control_values(&self.song),
                &control_values(&song),
                String::new(),
                &mut edited,
            );
            if !edited.is_empty() {
                let keys: HashSet<_> = song
                    .automation
                    .iter()
                    .map(|e| (e.track, e.controller))
                    .collect();
                let mut template = self.song.clone();
                template.patterns.clear();
                template.arrangement.clear();
                template.automation.clear();
                for (track, controller) in keys {
                    if self.manual.contains(&(track, controller)) {
                        continue;
                    }
                    let paths = automation_paths(&template, track, controller);
                    if !edited.is_disjoint(&paths) {
                        self.manual.insert((track, controller));
                        self.manual_paths.extend(paths);
                    }
                }
            }
        }
        self.song = song;
        if self.song.song_mode {
            self.bar = self.bar.min(self.song.arrangement.len().saturating_sub(1));
        }
        self.restore_automation(self.bar * 32 + self.half.min(31));
        if !self.playing {
            self.active_slots = self.desired_slots();
        }
        self.rack.configure(&self.current);
    }
    fn desired_slots(&self) -> [usize; 4] {
        if self.current.song_mode {
            self.current
                .arrangement
                .get(self.bar)
                .copied()
                .unwrap_or(self.current.slots)
        } else {
            self.current.slots
        }
    }
    fn restore_automation(&mut self, tick: usize) {
        self.current = self.song.clone();
        let mut preserved = self.manual_paths.clone();
        if self.song.song_mode {
            self.current.restore_control_base();
            if self.song.automation_base.is_some() {
                let mut untouched = HashSet::new();
                different_paths(
                    &control_values(&self.song),
                    &control_values(&self.current),
                    String::new(),
                    &mut untouched,
                );
                let keys: HashSet<_> = self.song.automation.iter().map(|e| (e.track, e.controller)).collect();
                let mut template = self.current.clone();
                template.patterns.clear();
                template.arrangement.clear();
                template.automation.clear();
                template.automation_base = None;
                for (track, controller) in keys {
                    let paths = automation_paths(&template, track, controller);
                    untouched.retain(|path| !paths.contains(path));
                }
                preserved.extend(untouched);
            }
        }
        self.auto_cursor = 0;
        if self.song.song_mode {
            while self.auto_cursor < self.song.automation.len()
                && (self.song.automation[self.auto_cursor].tick as usize) < tick
            {
                let e = &self.song.automation[self.auto_cursor];
                if !self.manual.contains(&(e.track, e.controller)) {
                    rb_model::rbs::apply_automation(&mut self.current, e);
                }
                self.auto_cursor += 1;
            }
        }
        // Restore only automated lanes; retain other settings and live edits.
        if !preserved.is_empty() {
            let source = control_values(&self.song);
            let mut target = control_values(&self.current);
            for path in &preserved {
                if let (Some(value), Some(slot)) = (source.pointer(path), target.pointer_mut(path))
                {
                    *slot = value.clone();
                }
            }
            let (synth, drums, channels, fx, enabled): (
                [rb_model::Synth; 2],
                [rb_model::DrumKit; 2],
                [rb_model::Channel; 4],
                rb_model::Effects,
                [bool; 4],
            ) = serde_json::from_value(target).expect("valid control types");
            self.current.synth = synth;
            self.current.drums = drums;
            self.current.channels = channels;
            self.current.fx = fx;
            self.current.enabled = enabled;
        }
        self.controls_revision = self.controls_revision.wrapping_add(1);
    }
    fn clear_sound(&mut self, echo: bool) {
        for voice in &mut self.voices {
            voice.clear();
        }
        for kit in &mut self.drums {
            kit.clear();
        }
        self.flams = [[[0.; 16]; 12]; 2];
        self.flam_gain = [[[0.; 16]; 12]; 2];
        self.click = 0.;
        self.rack.clear(echo);
        self.rack.configure(&self.current);
    }
    fn seek(&mut self, bar: usize) {
        self.bar = bar.min(self.song.arrangement.len().saturating_sub(1));
        self.half = 0;
        self.step = 0;
        self.time = 0.;
        self.next_half = 0.;
        self.track_time = [0.; 4];
        self.track_step = [0; 4];
        self.track_count = [0; 4];
        self.restore_automation(self.bar * 32);
        self.active_slots = self.desired_slots();
        self.clear_sound(true);
    }
    /// 0 Stop (echo tails retained), 1 Continue, 2 Seek bar, 3 audition,
    /// 4 metronome, 5 reset/panic, 6 recording, 7 live control (track << 8 | controller).
    pub fn command(&mut self, cmd: u32, value: u32) {
        match cmd {
            0 => {
                if !self.playing {
                    self.seek(if self.bar > self.song.start {
                        self.song.start
                    } else {
                        0
                    });
                }
                self.playing = false;
                self.manual.clear();
                self.manual_paths.clear();
                self.clear_sound(false);
            }
            1 => {
                if !self.playing {
                    self.manual.clear();
                    self.manual_paths.clear();
                    self.restore_automation(self.bar * 32 + self.half.min(31));
                    self.rack.configure(&self.current);
                    self.playing = true;
                }
            }
            2 => self.seek(value as usize),
            3 => {
                let k = (value / 16) as usize;
                let id = (value % 16) as usize;
                if k < 2 && id > 0 && id < 12 {
                    self.drums[k].trigger(id, 1., &self.current.drums[k]);
                }
            }
            4 => self.metronome = value != 0,
            5 => {
                self.playing = false;
                self.manual.clear();
                self.manual_paths.clear();
                self.seek(self.song.start);
            }
            6 => {
                self.recording = value != 0;
                self.manual.clear();
                self.manual_paths.clear();
                if !self.recording {
                    self.restore_automation(self.bar * 32 + self.half.min(31));
                    self.rack.configure(&self.current);
                }
            }
            7 => {
                let track = (value >> 8) as u8;
                let controller = value as u8;
                self.manual.insert((track, controller));
                let mut template = self.song.clone();
                template.patterns.clear();
                template.arrangement.clear();
                template.automation.clear();
                template.automation_base = None;
                self.manual_paths.extend(automation_paths(&template, track, controller));
            }
            9 => self.set_midi_sync(value != 0),
            10 => self.midi_clock(),
            11 => self.midi_start(),
            12 => self.midi_continue(),
            13 => self.midi_stop(),
            14 => self.midi_song_position(value),
            _ => {}
        }
    }
    /// Select external MIDI clock. Existing position is retained, aligned to its bar.
    pub fn set_midi_sync(&mut self, enabled: bool) {
        if self.midi_sync != enabled { self.midi_sync = enabled; self.seek(self.bar); }
    }
    /// A 24-PPQN pulse advances musical phase; rendered frames never advance it in this mode.
    pub fn midi_clock(&mut self) {
        if !self.midi_sync || !self.playing { return; }
        if self.midi_seen && self.midi_elapsed > 0. && self.midi_elapsed < 1. {
            self.current.bpm=(60. / (self.midi_elapsed * 24.)).clamp(20.,500.) as f32;
        }
        self.midi_seen=true;self.midi_elapsed=0.;
        self.advance_boundaries();
        if self.playing { self.time += 60. / 120. / 24.; }
    }
    pub fn midi_start(&mut self) {
        if !self.midi_sync { return; }
        self.manual.clear(); self.manual_paths.clear(); self.seek(0); self.midi_seen=false; self.midi_elapsed=0.; self.playing=true;
    }
    pub fn midi_continue(&mut self) { if self.midi_sync { self.playing=true; } }
    pub fn midi_stop(&mut self) {
        if self.midi_sync { self.playing=false; self.midi_seen=false; self.clear_sound(false); }
    }
    /// MIDI SPP units are six MIDI clocks (one sixteenth note).
    pub fn midi_song_position(&mut self, sixteenths: u32) {
        if !self.midi_sync { return; }
        let bar=(sixteenths as usize / 16).min(self.song.arrangement.len().saturating_sub(1));
        let within=if bar == sixteenths as usize / 16 {sixteenths as usize % 16} else {0};
        let was=self.playing;self.seek(bar);self.playing=true;
        for _ in 0..within*6 {self.midi_clock();}
        self.playing=was;self.clear_sound(true);
    }
    fn advance_boundaries(&mut self) {
        while self.time + 1e-10 >= self.next_half && self.playing { self.master_boundary(); }
        if self.playing { for t in 0..4 { while self.time + 1e-10 >= self.track_time[t] { self.track_boundary(t); } } }
    }
    pub fn position(&self) -> u32 {
        (self.bar * 16 + self.step) as u32
    }
    /// Current audible 32nd-note tick; `half` itself points to the next boundary.
    pub fn tick(&self) -> u32 {
        (self.bar * 32 + self.half.saturating_sub(1)) as u32
    }
    pub fn playing(&self) -> bool {
        self.playing
    }
    fn master_boundary(&mut self) {
        if self.half == 32 {
            self.half = 0;
            self.bar += 1;
            if self.current.song_mode {
                let end = self
                    .song
                    .start
                    .saturating_add(self.song.length)
                    .min(self.song.arrangement.len());
                if self.song.looping && self.bar == end {
                    self.bar = self.song.start;
                    self.restore_automation(self.bar * 32);
                }
                if self.bar >= self.song.arrangement.len() {
                    self.playing = false;
                    self.clear_sound(false);
                    return;
                }
            }
        }
        let tick = self.bar * 32 + self.half;
        if self.current.song_mode {
            let previous_cursor = self.auto_cursor;
            while self.auto_cursor < self.song.automation.len()
                && self.song.automation[self.auto_cursor].tick as usize <= tick
            {
                let e = &self.song.automation[self.auto_cursor];
                if !self.manual.contains(&(e.track, e.controller)) {
                    rb_model::rbs::apply_automation(&mut self.current, e);
                }
                self.auto_cursor += 1;
            }
            if previous_cursor != self.auto_cursor {
                self.controls_revision = self.controls_revision.wrapping_add(1);
            }
        }
        self.rack.tick(tick, &self.current);
        self.rack.configure(&self.current);
        self.step = self.half / 2;
        if self.metronome && self.half % 8 == 0 {
            self.click = 1.;
        }
        self.half += 1;
        self.next_half += 60. / if self.midi_sync {120.} else {self.current.bpm as f64} / 8.;
    }
    fn track_boundary(&mut self, t: usize) {
        let old = self.active_slots[t];
        if self.track_step[t] >= self.current.patterns[old].lengths[t] {
            self.track_step[t] = 0;
        }
        if self.track_step[t] == 0 {
            self.active_slots[t] = self.desired_slots()[t];
        }
        let slot = self.active_slots[t];
        let i = self.track_step[t];
        let swing = if self.current.shuffle_tracks[t] {
            self.current.shuffle / 100. * 0.9
        } else {
            0.
        };
        let duration = 60. / self.current.bpm / 4.
            * if self.track_count[t] % 2 == 0 {
                1. + swing
            } else {
                1. - swing
            };
        if self.current.enabled[t] {
            if t < 2 {
                self.voices[t].note(
                    &self.current.patterns[slot].notes[t][i],
                    &self.current.synth[t],
                    duration,
                    self.current.vintage,
                );
            } else {
                let k = t - 2;
                let p = &self.current.patterns[slot];
                let accent = if p.drums[k][0][i] > 0 {
                    1. + self.current.drums[k].voices[0].level * 0.65
                } else {
                    1.
                };
                // Order resolves simultaneous hats: 808 OH then CH; 909 CH then OH.
                for id in 1..12 {
                    let v = p.drums[k][id][i];
                    if v > 0 {
                        let gain = accent * if v == 2 { 1. } else { 0.72 };
                        self.drums[k].trigger(id, gain, &self.current.drums[k]);
                        if v == 3 {
                            // A new flam must not replace earlier delayed hits.
                            if let Some(index)=self.flams[k][id].iter().position(|time| *time<=0.) {
                                self.flams[k][id][index]=self.current.drums[k].flam*self.sr;
                                self.flam_gain[k][id][index]=gain*0.85;
                            }
                        }
                    }
                }
            }
        } else if t < 2 {
            self.voices[t].note(
                &rb_model::Note {
                    note: 36,
                    gate: false,
                    accent: false,
                    slide: false,
                },
                &self.current.synth[t],
                duration,
                self.current.vintage,
            );
        }
        self.track_step[t] += 1;
        self.track_count[t] += 1;
        self.track_time[t] += if self.midi_sync {60. / 120. / 4. * if self.track_count[t] % 2 == 1 {1. + swing as f64} else {1. - swing as f64}} else {duration as f64};
    }
    pub fn render(&mut self, out: &mut [f32]) {
        self.rack.metrics = [0.; 11];
        let dt = 1. / self.sr as f64;
        for frame in out.chunks_exact_mut(2) {
            if self.midi_sync {self.midi_elapsed += dt;}
            if self.playing && !self.midi_sync {
                self.advance_boundaries();
                self.time += dt;
            }
            let mut ch = [0.; 4];
            for i in 0..2 {
                ch[i] = self.voices[i].sample(&self.current.synth[i]);
            }
            for k in 0..2 {
                for id in 1..12 {
                    for index in 0..16 {
                        if self.flams[k][id][index]>0. {
                            self.flams[k][id][index]-=1.;
                            if self.flams[k][id][index]<=0. {
                                self.drums[k].trigger(id,self.flam_gain[k][id][index],&self.current.drums[k]);
                            }
                        }
                    }
                }
                ch[k + 2] = self.drums[k].sample(&self.current.drums[k]);
            }
            self.click_phase = (self.click_phase + 1800. / self.sr).fract();
            let click = (self.click_phase * std::f32::consts::TAU).sin() * self.click * 0.08;
            self.click *= 0.99;
            let samples = self.rack.sample(ch, &self.current, click);
            frame.copy_from_slice(&samples);
        }
    }
}

fn automation_paths(template: &Song, track: u8, controller: u8) -> HashSet<String> {
    let mut low = template.clone();
    let mut high = template.clone();
    rb_model::rbs::apply_automation(
        &mut low,
        &Automation { tick: 0, track, controller, value: 0 },
    );
    rb_model::rbs::apply_automation(
        &mut high,
        &Automation { tick: 0, track, controller, value: 127 },
    );
    let mut paths = HashSet::new();
    different_paths(&control_values(&low), &control_values(&high), String::new(), &mut paths);
    if track == 0 && (controller == 1 || controller == 2) {
        for value in 1..=5 {
            let mut destination = template.clone();
            rb_model::rbs::apply_automation(
                &mut destination,
                &Automation { tick: 0, track, controller, value },
            );
            different_paths(&control_values(&low), &control_values(&destination), String::new(), &mut paths);
        }
    }
    paths
}

fn control_values(s: &Song) -> serde_json::Value {
    serde_json::json!([s.synth, s.drums, s.channels, s.fx, s.enabled])
}
fn different_paths(
    a: &serde_json::Value,
    b: &serde_json::Value,
    path: String,
    out: &mut HashSet<String>,
) {
    match (a, b) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            for (k, v) in a {
                if let Some(w) = b.get(k) {
                    different_paths(v, w, format!("{path}/{k}"), out);
                }
            }
        }
        (serde_json::Value::Array(a), serde_json::Value::Array(b)) => {
            for (i, (v, w)) in a.iter().zip(b).enumerate() {
                different_paths(v, w, format!("{path}/{i}"), out);
            }
        }
        _ => {
            if a != b {
                out.insert(path);
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod abi {
    use super::*;
    const INPUT_CAP: usize = 16 * 1024 * 1024;
    const BLOCK: usize = 128;
    use std::cell::RefCell;
    thread_local! { static ENGINE: RefCell<Option<Engine>> = const { RefCell::new(None) }; }
    thread_local! { static CONTROLS: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) }; }
    // These buffers are accessed only by the serialized worklet on its own
    // WebAssembly instance. The host must not call exports concurrently.
    static mut INPUT: [u8; INPUT_CAP] = [0; INPUT_CAP];
    static mut OUTPUT: [f32; BLOCK * 2] = [0.0; BLOCK * 2];
    #[no_mangle]
    pub extern "C" fn rb_init(sample_rate: f32) {
        ENGINE.with(|e| *e.borrow_mut() = Some(Engine::new(sample_rate)));
    }
    #[no_mangle]
    pub extern "C" fn rb_input_ptr() -> *mut u8 {
        std::ptr::addr_of_mut!(INPUT).cast()
    }
    #[no_mangle]
    pub extern "C" fn rb_input_capacity() -> usize {
        INPUT_CAP
    }
    #[no_mangle]
    pub extern "C" fn rb_set_song(len: usize) -> i32 {
        if len > INPUT_CAP {
            return -1;
        }
        // The input buffer belongs to this instance and is not changed during this call.
        let bytes =
            unsafe { std::slice::from_raw_parts(std::ptr::addr_of!(INPUT).cast::<u8>(), len) };
        if let Ok(text) = std::str::from_utf8(bytes) {
            if let Ok(song) = Song::from_json(text) {
                return ENGINE.with(|e| {
                    if let Some(engine) = e.borrow_mut().as_mut() {
                        engine.set_song(song);
                        0
                    } else {
                        -1
                    }
                });
            }
        }
        -1
    }
    #[no_mangle]
    pub extern "C" fn rb_command(command: u32, value: u32) {
        ENGINE.with(|e| {
            if let Some(engine) = e.borrow_mut().as_mut() {
                engine.command(command, value);
            }
        });
    }
    #[no_mangle]
    pub extern "C" fn rb_render(frames: usize) {
        // The host reads this output only after the serialized render call returns.
        let out = unsafe {
            std::slice::from_raw_parts_mut(std::ptr::addr_of_mut!(OUTPUT).cast::<f32>(), BLOCK * 2)
        };
        out.fill(0.0);
        ENGINE.with(|e| {
            if let Some(engine) = e.borrow_mut().as_mut() {
                engine.render(&mut out[..frames.min(BLOCK) * 2]);
            }
        });
    }
    #[no_mangle]
    pub extern "C" fn rb_output_ptr() -> *const f32 {
        std::ptr::addr_of!(OUTPUT).cast()
    }
    #[no_mangle]
    pub extern "C" fn rb_position() -> u32 {
        ENGINE.with(|e| e.borrow().as_ref().map_or(0, |e| e.position()))
    }
    #[no_mangle]
    pub extern "C" fn rb_tick() -> u32 {
        ENGINE.with(|e| e.borrow().as_ref().map_or(0, |e| e.tick()))
    }
    #[no_mangle]
    pub extern "C" fn rb_metrics_ptr() -> *const f32 {
        ENGINE.with(|e| {
            e.borrow()
                .as_ref()
                .map_or(std::ptr::null(), |e| e.rack.metrics.as_ptr())
        })
    }
    #[no_mangle]
    pub extern "C" fn rb_playing() -> u32 {
        ENGINE.with(|e| e.borrow().as_ref().map_or(0, |e| e.playing as u32))
    }
    #[no_mangle]
    pub extern "C" fn rb_controls_revision() -> u32 {
        ENGINE.with(|e| e.borrow().as_ref().map_or(0, |e| e.controls_revision))
    }
    #[no_mangle]
    pub extern "C" fn rb_controls_len() -> usize {
        ENGINE.with(|e|if let Some(engine)=e.borrow().as_ref() {
            let s=&engine.current;
            let bytes=serde_json::to_vec(&serde_json::json!({"synth":s.synth,"drums":s.drums,"channels":s.channels,"fx":s.fx,"enabled":s.enabled,"shuffle_tracks":s.shuffle_tracks,"vintage":s.vintage})).expect("finite controls");
            CONTROLS.with(|c|{*c.borrow_mut()=bytes;c.borrow().len()})
        }else{0})
    }
    #[no_mangle]
    pub extern "C" fn rb_controls_ptr() -> *const u8 {
        CONTROLS.with(|c| c.borrow().as_ptr())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unused_recording_baseline_preserves_later_edits() {
        let mut song = Song::default();
        song.song_mode = true;
        song.bpm = 500.;
        song.automation_base = Some(song.control_state());
        song.synth[0].cutoff = 3100.;
        song.drums[1].voices[1].level = 0.4;
        song.channels[2].pan = -0.6;
        song.fx.filter_pattern = 20;
        let song = Song::from_json(&serde_json::to_string(&song).unwrap()).unwrap();
        let expected = song.control_state();
        let mut engine = Engine::new(8000.);
        engine.set_song(song);
        assert_eq!(engine.current.control_state(), expected);
        engine.command(1, 0);
        render(&mut engine, 1.1);
        assert_eq!(engine.current.control_state(), expected);
        engine.command(0, 0);
        engine.command(1, 0);
        assert_eq!(engine.current.control_state(), expected);
    }
    #[test]
    fn baseline_restores_only_automated_controls() {
        let mut song = Song::default();
        song.song_mode = true;
        song.automation_base = Some(song.control_state());
        let cutoff = song.synth[0].cutoff;
        song.synth[0].cutoff = 4700.;
        song.synth[1].resonance = 0.8;
        song.automation.push(Automation { tick: 4, track: 1, controller: 3, value: 127 });
        let mut engine = Engine::new(8000.);
        engine.set_song(song);
        assert_eq!(engine.current.synth[0].cutoff, cutoff);
        assert_eq!(engine.current.synth[1].resonance, 0.8);
        engine.command(1, 0);
        render(&mut engine, 0.3);
        assert_eq!(engine.current.synth[0].cutoff, 6000.);
        assert_eq!(engine.current.synth[1].resonance, 0.8);
    }
    #[test]
    fn explicit_live_edit_can_match_the_saved_value() {
        let mut song = Song::default();
        song.song_mode = true;
        song.automation_base = Some(song.control_state());
        song.automation.push(Automation { tick: 0, track: 1, controller: 0, value: 0 });
        let mut engine = Engine::new(8000.);
        engine.set_song(song.clone());
        engine.command(1, 0);
        render(&mut engine, 0.1);
        assert!(!engine.current.enabled[0]);
        engine.command(7, 1 << 8);
        engine.set_song(song);
        assert!(engine.current.enabled[0]);
        render(&mut engine, 0.1);
        assert!(engine.current.enabled[0]);
        engine.command(0, 0);
        engine.command(1, 0);
        assert!(!engine.current.enabled[0]);
    }
    #[test]
    fn replacing_song_before_reset_uses_its_own_start() {
        let mut old = Song::default();
        old.song_mode = true;
        old.arrangement = vec![[0, 1, 2, 3]; 8];
        old.start = 4;
        let mut engine = Engine::new(8000.);
        engine.set_song(old);
        engine.command(5, 0);
        assert_eq!(engine.position(), 64);
        let mut incoming = Song::default();
        incoming.song_mode = true;
        incoming.start = 1;
        incoming.length = 1;
        engine.set_song(incoming);
        engine.command(5, 0);
        assert_eq!(engine.position(), 16);
        assert_eq!(engine.tick(), 32);
    }
    #[test]
    fn recording_position_includes_odd_32nd_ticks() {
        let mut e = Engine::new(48000.);
        let mut song = Song::default();
        song.bpm = 120.;
        e.set_song(song);
        assert_eq!(e.tick(), 0);
        e.command(1, 0);
        render(&mut e, 0.07);
        assert_eq!(e.position(), 0);
        assert_eq!(e.tick(), 1);
        render(&mut e, 0.06);
        assert_eq!(e.position(), 1);
        assert_eq!(e.tick(), 2);
        e.command(2, 1);
        assert_eq!(e.tick(), 32);
    }
    fn render(e: &mut Engine, s: f32) -> Vec<f32> {
        let mut out = vec![0.; (s * e.sr) as usize * 2];
        e.render(&mut out);
        assert!(out.iter().all(|x| x.is_finite()));
        out
    }
    #[test]
    fn pattern_always_loops() {
        let mut e = Engine::new(8000.);
        e.song.looping = false;
        e.song.bpm = 500.;
        e.command(1, 0);
        render(&mut e, 3.);
        assert!(e.playing);
        assert!(e.bar > 1);
    }
    #[test]
    fn song_range_and_continue() {
        let mut e = Engine::new(8000.);
        e.song.song_mode = true;
        e.song.bpm = 500.;
        e.song.arrangement = vec![[0, 1, 2, 3]; 6];
        e.song.start = 2;
        e.song.length = 2;
        e.command(2, 2);
        e.command(1, 0);
        assert_eq!(e.bar, 2);
        render(&mut e, 1.95);
        assert_eq!(e.bar, 2);
        e.song.looping = false;
        e.command(2, 4);
        render(&mut e, 1.);
        assert!(!e.playing);
    }
    #[test]
    fn lengths_cross_bar() {
        let mut e = Engine::new(8000.);
        e.song.patterns[0].lengths[0] = 7;
        e.song.bpm = 120.;
        e.command(1, 0);
        render(&mut e, 2.01);
        assert_eq!(e.track_count[0], 17);
        assert_eq!(e.track_step[0], 3);
    }
    #[test]
    fn all_drums_and_panic() {
        let mut e = Engine::new(8000.);
        for k in 0..2 {
            for id in 1..12 {
                e.command(5, 0);
                e.command(3, (k * 16 + id) as u32);
                assert!(
                    render(&mut e, 0.1).iter().any(|x| x.abs() > 1e-7),
                    "{k}/{id}"
                );
                e.command(5, 0);
                assert!(render(&mut e, 0.01).iter().all(|x| *x == 0.));
            }
        }
    }
    #[test]
    fn stopped_automation_does_not_run_in_pattern_mode() {
        let mut e = Engine::new(8000.);
        e.song.automation.push(Automation {
            tick: 0,
            track: 1,
            controller: 3,
            value: 0,
        });
        e.command(1, 0);
        render(&mut e, 0.1);
        assert_eq!(e.current.synth[0].cutoff, e.song.synth[0].cutoff);
    }
    #[test]
    fn recorded_baseline_and_loop_replay() {
        let mut s = Song::default();
        s.song_mode = true;
        s.looping = true;
        s.start = 0;
        s.length = 2;
        s.bpm = 500.;
        s.automation_base = Some(s.control_state());
        let baseline = s.synth[0].cutoff;
        s.automation.push(Automation {
            tick: 32,
            track: 1,
            controller: 3,
            value: 100,
        });
        let mut e = Engine::new(8000.);
        e.set_song(s.clone());
        e.command(1, 0);
        render(&mut e, 0.1);
        assert_eq!(e.current.synth[0].cutoff, baseline);
        e.command(6, 1);
        let mut edit = s.clone();
        edit.synth[0].cutoff = 4700.;
        e.set_song(edit);
        assert!(e.manual.contains(&(1, 3)));
        e.command(6, 0);
        render(&mut e, 0.45);
        let expected = 100. + 100. / 127. * 5900.;
        assert!((e.current.synth[0].cutoff - expected).abs() < 0.01);
        render(&mut e, 0.5);
        assert_eq!(e.bar, 0);
        assert_eq!(e.current.synth[0].cutoff, baseline);
        render(&mut e, 0.5);
        assert!((e.current.synth[0].cutoff - expected).abs() < 0.01);
    }
    #[test]
    fn manual_routing_and_cutoff_survive_automation_base() {
        let mut s = Song::default();
        s.song_mode = true;
        s.automation_base = Some(s.control_state());
        s.automation = vec![
            Automation {
                tick: 10,
                track: 0,
                controller: 1,
                value: 3,
            },
            Automation {
                tick: 10,
                track: 0,
                controller: 2,
                value: 3,
            },
            Automation {
                tick: 10,
                track: 1,
                controller: 3,
                value: 90,
            },
        ];
        let mut e = Engine::new(8000.);
        e.set_song(s.clone());
        e.command(1, 0);
        render(&mut e, 0.01);
        s.channels[0].compressor = true;
        s.channels[0].filter = true;
        s.synth[0].cutoff = 3000.;
        e.set_song(s);
        assert!(e.manual.contains(&(0, 1)));
        assert!(e.manual.contains(&(0, 2)));
        assert!(e.manual.contains(&(1, 3)));
        assert!(e.current.channels[0].compressor);
        assert!(e.current.channels[0].filter);
        assert_eq!(e.current.synth[0].cutoff, 3000.);
        render(&mut e, 0.8);
        assert!(e.current.channels[0].compressor);
        assert!(e.current.channels[0].filter);
        assert_eq!(e.current.synth[0].cutoff, 3000.);
    }
    #[test]
    fn stop_continue_retains_midbar_clocks() {
        let mut e = Engine::new(8000.);
        e.command(1, 0);
        render(&mut e, 0.37);
        let snapshot = (
            e.bar,
            e.half,
            e.step,
            e.time,
            e.next_half,
            e.track_time,
            e.track_step,
            e.track_count,
        );
        e.command(0, 0);
        e.command(1, 0);
        assert_eq!(
            snapshot,
            (
                e.bar,
                e.half,
                e.step,
                e.time,
                e.next_half,
                e.track_time,
                e.track_step,
                e.track_count
            )
        );
        render(&mut e, 0.05);
        assert!(e.time > snapshot.3);
        assert!(e.step >= snapshot.2);
    }
    #[test]
    fn disabled_tied_acid_releases() {
        let mut s = Song::default();
        s.channels[1].mute = true;
        s.channels[2].mute = true;
        s.channels[3].mute = true;
        s.fx.delay_enabled = false;
        s.fx.filter_enabled = false;
        s.fx.compressor_enabled = false;
        for n in &mut s.patterns[0].notes[0] {
            n.gate = true;
            n.slide = true;
        }
        let mut e = Engine::new(8000.);
        e.set_song(s.clone());
        e.command(1, 0);
        assert!(render(&mut e, 0.1).iter().any(|x| x.abs() > 1e-5));
        s.enabled[0] = false;
        e.set_song(s);
        let out = render(&mut e, 0.4);
        assert!(out[out.len() - 512..].iter().all(|x| x.abs() < 1e-6));
    }
    #[test]
    fn repeated_flams_keep_earlier_delayed_hits() {
        let mut s=Song::default();s.bpm=500.;s.drums[0].flam=0.2;s.patterns[s.slots[2]].drums[0][1].fill(3);
        let mut e=Engine::new(8000.);e.set_song(s);e.command(1,0);render(&mut e,0.1);
        assert_eq!(e.flams[0][1].iter().filter(|t|**t>0.).count(),4);
        render(&mut e,0.11);assert_eq!(e.flams[0][1].iter().filter(|t|**t>0.).count(),6);
    }

    #[test]
    fn record_punch_suppresses_old_automation_until_punch_out() {
        let mut s=Song::default();s.song_mode=true;s.bpm=120.;s.automation_base=Some(s.control_state());
        s.automation=vec![Automation{tick:4,track:1,controller:3,value:10},Automation{tick:16,track:1,controller:3,value:127}];
        let mut e=Engine::new(8000.);e.set_song(s.clone());e.command(1,0);e.command(6,1);render(&mut e,0.05);
        s.synth[0].cutoff=3000.;s.automation.push(Automation{tick:1,track:1,controller:3,value:62});s.automation.sort_by_key(|x|x.tick);e.set_song(s.clone());
        assert!(e.manual.contains(&(1,3)));render(&mut e,0.3);assert_eq!(e.current.synth[0].cutoff,3000.);
        s.automation.retain(|event|event.tick!=4);e.set_song(s);e.command(6,0);render(&mut e,0.8);
        assert!((e.current.synth[0].cutoff-6000.).abs()<0.01);
    }

}
#[cfg(test)] mod midi_sync_tests {
 use super::*;
 #[test] fn pulses_own_phase_and_transport(){let mut e=Engine::new(8000.);e.set_midi_sync(true);e.midi_start();let mut out=[0.;256];for _ in 0..100{e.render(&mut out)}assert_eq!(e.half,0);assert_eq!(e.track_count,[0;4]);for _ in 0..7{e.midi_clock()}assert_eq!(e.half,3);assert_eq!(e.track_count,[2;4]);let phase=e.half;e.midi_stop();for _ in 0..30{e.midi_clock()}assert_eq!(e.half,phase);e.midi_continue();for _ in 0..3{e.midi_clock()}assert_eq!(e.half,4);e.midi_song_position(20);assert_eq!(e.bar,1);assert_eq!(e.half,8);e.midi_clock();assert_eq!(e.half,9);}
 #[test] fn pattern_length_and_shuffle_remain_clocked(){let mut e=Engine::new(8000.);e.song.patterns[e.song.slots[0]].lengths[0]=3;e.song.shuffle=50.;e.set_midi_sync(true);e.midi_start();for _ in 0..25{e.midi_clock()}assert_eq!(e.track_count[0],5);assert_eq!(e.track_step[0],2);}
}
