//! Procedural percussion and reference sample-bank voices.
use rb_model::DrumKit;
use std::f32::consts::TAU;
#[path = "percussion_color.rs"]
mod color;
#[path = "percussion_kicks.rs"]
mod kicks;
#[path = "percussion_modes.rs"]
mod membranes;

#[derive(Clone, Copy)]
struct Voice {
    env: f32,
    body: f32,
    noise: f32,
    age: u32,
    hold: u32,
    colored: bool,
    banked: bool,
    seed: u32,
    phase: [f32; 6],
    freq: [f32; 6],
    fall: f32,
    body_fall: f32,
    noise_fall: f32,
    noise_cut: f32,
    pitch: f32,
    pitch_fall: f32,
    tone_state: f32,
    tone_setting: f32,
    band: Band,
    banks: [Band; 8],
    color_gain: [f32; 8],
    color_fall: [f32; 8],
    color_hp: [[f32; 2]; 8],
    color_hp_alpha: f32,
    source_step: f64,
    source_phase: f64,
    source_age: u32,
    source_pair: [f32; 2],
    modes: [Mode; 8],
    mode_count: u8,
    mode_pulse: [f32; 2],
    mode_pulse_fall: [f32; 2],
    mode_end: f32,
    velocity: f32,
    variant: bool,
}
impl Default for Voice {
    fn default() -> Self {
        Self {
            env: 0.,
            body: 0.,
            noise: 0.,
            age: 0,
            hold: 0,
            colored: false,
            banked: false,
            seed: 1,
            phase: [0.; 6],
            freq: [0.; 6],
            fall: 0.,
            body_fall: 0.,
            noise_fall: 0.,
            noise_cut: 0.,
            pitch: 0.,
            pitch_fall: 0.,
            tone_state: 0.,
            tone_setting: f32::NAN,
            band: Band::default(),
            banks: [Band::default(); 8],
            color_gain: [0.; 8],
            color_fall: [0.; 8],
            color_hp: [[0.; 2]; 8],
            color_hp_alpha: 0.,
            source_step: 0.,
            source_phase: 0.,
            source_age: 0,
            source_pair: [0.; 2],
            modes: [Mode::default(); 8],
            mode_count: 0,
            mode_pulse: [0.; 2],
            mode_pulse_fall: [0.; 2],
            mode_end: 0.,
            velocity: 0.,
            variant: false,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Mode {
    phase: f32,
    step: f32,
    amp: f32,
    fall: f32,
    pitch: f32,
    pitch_fall: f32,
    curve: f32,
}
#[derive(Clone, Copy, Default)]
struct Band {
    b: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}
impl Band {
    fn new(hz: f32, q: f32, sr: f32) -> Self {
        let w = TAU * hz.min(sr * 0.45) / sr;
        let a = w.sin() / (2. * q);
        let n = 1. + a;
        Self {
            b: a / n,
            a1: -2. * w.cos() / n,
            a2: (1. - a) / n,
            ..Self::default()
        }
    }
    fn process(&mut self, x: f32) -> f32 {
        let y = self.b * (x - self.x2) - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

fn colored_noise(v: &mut Voice, decay: bool) -> f32 {
    let mut out = 0.;
    for i in 0..8 {
        v.seed ^= v.seed << 13;
        v.seed ^= v.seed >> 17;
        v.seed ^= v.seed << 5;
        let noise = v.seed as f32 / u32::MAX as f32 * 2. - 1.;
        let state = &mut v.color_hp[i];
        state[0] += v.color_hp_alpha * (noise - state[0]);
        let high = noise - state[0];
        state[1] += v.color_hp_alpha * (high - state[1]);
        out += v.banks[i].process(high - state[1]) * v.color_gain[i];
        if decay {
            v.color_gain[i] *= v.color_fall[i];
        }
    }
    out
}

fn colored_signal(v: &mut Voice, t: f32, kit: usize, id: usize) -> f32 {
    let [_, _, attack, _, hold, end] = v.freq;
    if t >= end {
        if v.colored {
            v.env = 0.;
            v.body = 0.;
        } else {
            v.noise = 0.;
        }
        return 0.;
    }
    let excitation = colored_noise(v, t >= hold);
    let mut envelope = (t / attack).min(1.) * ((end - t) / 0.01).clamp(0., 1.);
    if kit == 0 && id == 7 && !v.variant {
        envelope = 0.;
        for (start, amp) in [(0., 1.), (0.011, 0.8), (0.023, 0.7), (0.033, 0.9)] {
            if t >= start {
                envelope += amp * (-(t - start) / if start < 0.03 { 0.003 } else { 0.025 }).exp();
            }
        }
    }
    if kit == 1 && id == 7 {
        let burst = if t < 0.008 {
            0.4
        } else if (0.014..0.022).contains(&t) {
            0.65
        } else if (0.030..0.045).contains(&t) {
            1.
        } else {
            0.
        };
        envelope = burst
            + 0.35 * (-std::f32::consts::LN_10 * t / 0.14).exp()
            + 0.1 * (-std::f32::consts::LN_10 * t / 0.35).exp();
    }
    if v.colored {
        v.body = v.color_gain.iter().copied().fold(0., f32::max) * envelope * v.env;
        if t > 0.1 && v.body < 1e-7 {
            v.body = 0.;
            v.env = 0.;
        }
    }
    excitation * envelope
}

fn pitched_color(v: &mut Voice, kit: usize, id: usize) -> f32 {
    if v.source_age == 0 {
        v.source_pair[0] = colored_signal(v, 0., kit, id);
        v.source_pair[1] = colored_signal(v, 1. / 44100., kit, id);
        v.source_age = 2;
    }
    let out = v.source_pair[0] + (v.source_pair[1] - v.source_pair[0]) * v.source_phase as f32;
    v.source_phase += v.source_step;
    while v.source_phase >= 1. {
        v.source_phase -= 1.;
        v.source_pair[0] = v.source_pair[1];
        v.source_pair[1] = colored_signal(v, v.source_age as f32 / 44100., kit, id);
        v.source_age = v.source_age.saturating_add(1);
    }
    out
}

fn membrane_sample(v: &mut Voice, sr: f32) -> f32 {
    let t = v.age as f32 / sr;
    let mut out = 0.;
    let mut envelope = 0.;
    for mode in &mut v.modes[..v.mode_count as usize] {
        let amplitude = mode.amp * (-mode.curve * t * t).exp();
        out += (TAU * mode.phase).sin() * amplitude;
        envelope += amplitude.abs();
        mode.phase = (mode.phase + mode.step + mode.pitch).fract();
        mode.pitch *= mode.pitch_fall;
        mode.amp *= mode.fall;
    }
    for i in 0..2 {
        out += v.mode_pulse[i];
        envelope += v.mode_pulse[i].abs();
        v.mode_pulse[i] *= v.mode_pulse_fall[i];
    }
    let fade = ((v.mode_end - t) / 0.005).clamp(0., 1.);
    v.body = envelope * fade * v.env;
    if t >= v.mode_end || v.body < 1e-7 {
        v.body = 0.;
        v.env = 0.;
    }
    out * fade * v.env
}

struct SampleVoice {
    sample: std::sync::Arc<rb_model::samples::CustomSample>,
    position: f64,
    velocity: f32,
    env: f32,
    choke: f32,
    noise: Option<std::sync::Arc<rb_model::samples::CustomSample>>,
    bank_pitch: Option<f64>,
    noise_mix: f32,
    bank_gain: f32,
    bank_pitched: bool,
    bank_ended: bool,
    bank_env: f64,
    bank_fall: f32,
    bank_hold: f64,
    bank_clock: f64,
    filter_pole: Option<f32>,
    filter_state: f64,
}
pub struct DrumSynth {
    sr: f32,
    kit: usize,
    voices: [Voice; 12],
    kick_tail: Option<Voice>,
    samples: [Option<SampleVoice>; 12],
    bank_kick_tail: Option<SampleVoice>,
    snare_count: usize,
}
impl DrumSynth {
    pub fn new(sr: f32, kit: usize) -> Self {
        let sr = if sr.is_finite() {
            sr.clamp(8000., 192000.)
        } else {
            44100.
        };
        Self {
            sr,
            kit: kit.min(1),
            voices: [Voice::default(); 12],
            kick_tail: None,
            samples: std::array::from_fn(|_| None),
            bank_kick_tail: None,
            snare_count: 0,
        }
    }
    pub fn clear(&mut self) {
        self.voices = [Voice::default(); 12];
        self.kick_tail = None;
        self.bank_kick_tail = None;
        self.samples = std::array::from_fn(|_| None);
    }
    pub fn trigger_sample(&mut self, id: usize, velocity: f32, kit: &DrumKit, sample: Option<&std::sync::Arc<rb_model::samples::CustomSample>>) {
        if id == 0 || id >= 12 || kit.voices.len() != 12 || !velocity.is_finite() { return; }
        let open = if self.kit == 0 && id == 11 { Some(10) } else if self.kit == 1 && id == 8 { Some(9) } else { None };
        if let Some(open) = open { if let Some(v) = &mut self.samples[open] { if v.bank_pitch.is_some() { v.bank_hold = 0.; if self.kit == 1 { v.bank_fall = 0.996; } } else { v.choke = 0.998_f32.powf(44100. / (2. * self.sr)); } } }
        self.samples[id] = None;
        self.trigger(id, velocity, kit);
        if let Some(sample) = sample {
            self.voices[id] = Voice::default();
            if id == 1 { self.kick_tail = None; self.bank_kick_tail = None; }
            self.samples[id] = Some(SampleVoice { sample: sample.clone(), position: 0., velocity: velocity.clamp(0.,64.), env: 1., choke: 1., noise: None, bank_pitch: None, noise_mix: 0., bank_gain: 0., bank_pitched: false, bank_ended: false, bank_env: 1., bank_fall: 1., bank_hold: 0., bank_clock: 0., filter_pole: None, filter_state: 0. });
        }
    }
    // Resource mapping and step quantization follow OpenRB-338, GPL-3.0,
    // commit cbca4492c0224dffaa477346bc8860559f3d853f, tr808.rs/tr909.rs.
    pub fn trigger_bank(&mut self, id: usize, velocity: f32, kit: &DrumKit,
        manual: Option<&std::sync::Arc<rb_model::samples::CustomSample>>,
        bank: Option<&std::sync::Arc<rb_model::samples::SampleBank>>) {
        if manual.is_some() || bank.is_none() || id == 0 || id >= 12 || kit.voices.len() != 12 {
            self.trigger_sample(id, velocity, kit, manual);
            return;
        }
        let d = &kit.voices[id];
        let raw = (((d.tune + 6.) / 12. * 127.).round() as usize).min(127);
        let tone = (d.tone * 127.).round().clamp(0.,127.) as usize;
        let (resource, noise, pitched) = if self.kit == 0 {
            match id {
                1 => (0,None,false),
                2 => { const SEQ: [usize;32] = [0,1,2,3,1,0,3,1,0,3,2,1,0,2,3,1,0,2,3,2,0,1,2,0,3,1,2,0,1,3,2,1];
                    self.snare_count=(self.snare_count+1)%32;
                    (1+tone*5/128,Some(6+SEQ[self.snare_count]),false) },
                3 => (if d.variant {17} else {18},None,true),
                4 => (if d.variant {20} else {21},None,true),
                5 => (if d.variant {15} else {16},None,true),
                6 => (if d.variant {12} else {23},None,false),
                7 => (if d.variant {19} else {13},None,false),
                8 => (10,None,false), 9 => (14,None,false),
                10 => (22,None,false), 11 => (11,None,false),
                _ => unreachable!(),
            }
        } else {
            match id {
                1 => (25+raw/32,None,false),
                2 => { const SEQ: [usize;32] = [0,1,2,0,1,0,2,1,0,1,2,1,0,2,0,1,0,2,0,2,0,1,2,0,2,1,2,0,2,1,2,1];
                    self.snare_count=(self.snare_count+1)%32;
                    (29+raw*5/128,Some(34+SEQ[self.snare_count]),false) },
                3..=5 => (34+id,None,true), 6..=11 => (34+id,None,id>=10),
                _ => unreachable!(),
            }
        };
        let bank=bank.unwrap();
        let primary=bank.samples.get(resource).and_then(Option::as_ref);
        let secondary=noise.and_then(|i| bank.samples.get(i).and_then(Option::as_ref));
        if primary.is_none() || (noise.is_some() && secondary.is_none()) {
            self.trigger_sample(id, velocity, kit, None);
            return;
        }
        let mut tail = if id == 1 { self.samples[1].take().filter(|v| v.bank_pitch.is_some()) } else { None };
        if let Some(v) = &mut tail { v.bank_fall = 0.99; v.bank_hold = 0.; }
        self.trigger_sample(id, velocity, kit, primary);
        if id == 1 { self.bank_kick_tail = tail; }
        let sample = self.samples[id].as_mut().unwrap();
        sample.noise = secondary.cloned();
        let snappy = (d.snappy.clamp(0., 1.) * 127.).round() as f64;
        sample.noise_mix = if self.kit == 0 { (snappy * f32::from_bits(0x3c01_0204) as f64).powi(2) as f32 } else { (snappy * f32::from_bits(0x3c23_d70a) as f64) as f32 };
        let exponent = if self.kit == 0 {192.} else {144.};
        sample.bank_pitched = pitched;

        sample.bank_fall = if id == 1 || (self.kit == 0 && id == 9) || (self.kit == 1 && (3..=5).contains(&id)) || (self.kit == 1 && (id == 8 || id == 9)) {
            bank_decay(d.decay, if self.kit == 0 { 0 } else if id == 8 { 8 } else { 1 })
        } else if self.kit == 0 && id == 10 { 0.998 } else if self.kit == 1 && id == 2 { 0.998 } else { 1. };
        sample.bank_hold = if self.kit == 0 && id == 10 { 4. * (((d.decay - 0.3) / 1.7).clamp(0., 1.) * 127.).round() as f64 * 64. } else if self.kit == 1 && id == 2 { (tone + 30) as f64 * 64. } else { 0. };
        if id == 1 || (self.kit == 0 && id == 9) {
            sample.filter_pole = Some(if self.kit == 0 { [0.9939_f32, 0.9757, 0.906, 0.822, 0.675, 0.456, 0.208, 0.001][(tone >> 4).min(7)] } else {
                [0.92_f32, 0.87, 0.8, 0.72, 0.62, 0.5, 0.3, 0.001][((d.attack.clamp(0.,1.) * 8.) as usize).min(7)]
            });
        }
        let gain = bank_trigger_gain(self.kit, id, d.level, velocity.clamp(0., 64.), sample.filter_pole);
        sample.bank_gain = (gain as f64 / 32768.) as f32;
        sample.bank_env = if secondary.is_some() { sample.noise_mix as f64 } else { sample.bank_gain as f64 };
        sample.bank_pitch = Some(if pitched { if raw == 64 {1.} else {(256. * 2_f64.powf((raw as f64 - 64.) / exponent)).floor() / 256.} } else {1.});
    }
    pub fn trigger(&mut self, id: usize, velocity: f32, kit: &DrumKit) {
        if id == 0 || id >= 12 || kit.voices.len() != 12 || !velocity.is_finite() {
            return;
        }
        let d = &kit.voices[id];
        let sr = self.sr;
        let gain = velocity.clamp(0., 64.);
        // A closed hat shortens the open tail; it does not erase the other voice.
        if self.kit == 0 {
            if id == 11 {
                self.voices[10].hold = 0;
                self.voices[10].fall = 0.998_f32.powf(44100. / (2. * sr));
            }
        } else {
            if id == 8 {
                self.voices[9].hold = 0;
                self.voices[9].fall = 0.996_f32.powf(44100. / (2. * sr));
            }
        }
        let mut v = Voice {
            env: 1.,
            body: 1.,
            noise: 1.,
            velocity: gain,
            variant: d.variant,
            ..Voice::default()
        };
        if id == 1 {
            let raw = (((d.tune + 6.) / 12. * 127.).round() as usize).min(127);
            let shape = &kicks::KICKS[if self.kit == 0 { 0 } else { 1 + raw / 32 }];
            let [base, fast, fast_time, slow, slow_time] = shape.pitch;
            v.pitch = 1.;
            v.noise = 1.;
            v.pitch_fall = (-1. / (fast_time * sr)).exp();
            v.noise_fall = (-1. / (slow_time * sr)).exp();
            let alpha = if self.kit == 0 {
                tone_alpha(d.tone, sr)
            } else {
                let index = ((d.attack.clamp(0., 1.) * 8.) as usize).min(7);
                1. - [0.92_f32, 0.87, 0.8, 0.72, 0.62, 0.5, 0.3, 0.001][index].powf(44100. / sr)
            };
            v.freq = [
                base / sr,
                fast * fast_time * (1. - v.pitch_fall),
                slow * slow_time * (1. - v.noise_fall),
                shape.envelope[0],
                shape.envelope[1],
                alpha,
            ];
            for (mode, [amp, phase]) in v.modes.iter_mut().zip(shape.harmonics) {
                mode.amp = amp;
                mode.phase = phase;
            }
            v.mode_pulse = shape.pulse;
            v.mode_pulse_fall = [(-1. / (0.002 * sr)).exp(), (-1. / (0.010 * sr)).exp()];
            v.mode_end = shape.end;
            v.tone_setting = d.tone;
            v.fall = drum_decay(d.decay, sr, self.kit);
            // Alternate two kicks so a retrigger fades the previous tail instead of cutting it.
            let mut old = self.voices[1];
            old.fall = 0.99_f32.powf(44100. / (2. * sr));
            old.hold = 0;
            self.kick_tail = Some(old);
        } else if self.kit == 0 && id == 8 {
            v.freq[0] = 537.279;
            v.freq[1] = 809.69;
            v.band = Band::new(850., 2.3, sr);
            v.body = 0.09644592;
            v.noise = 0.5257373;
            v.body_fall = (-6.587016 / sr).exp();
            v.noise_fall = (-90.97862 / sr).exp();
            v.fall = 1.;
            v.mode_end = 1.1609977;
        }
        if let Some(shape) = membranes::shape(self.kit, id, d.variant, d.tune) {
            let raw = ((d.tune + 6.) / 12. * 127.).round();
            let speed = if (3..=5).contains(&id) {
                (256. * 2.0_f32.powf((raw - 64.) / if self.kit == 0 { 192. } else { 144. })).floor()
                    / 256.
            } else {
                1.
            };
            v.mode_count = shape.modes.len() as u8;
            for (mode, params) in v.modes.iter_mut().zip(shape.modes) {
                let [hz, pitch, tau, rate, curve, amp, phase] = *params;
                *mode = Mode {
                    phase: phase.rem_euclid(1.),
                    step: hz * speed / sr,
                    amp,
                    fall: (-rate * speed / sr).exp(),
                    pitch: pitch * speed / sr,
                    pitch_fall: (-speed / (tau * sr)).exp(),
                    curve: curve * speed * speed,
                };
            }
            v.mode_pulse = shape.pulse;
            v.mode_pulse_fall = [(-speed / (0.001 * sr)).exp(), (-speed / (0.005 * sr)).exp()];
            v.mode_end = shape.end / speed;
            v.fall = if self.kit == 1 && (3..=5).contains(&id) {
                drum_decay(d.decay, sr, 1)
            } else {
                1.
            };
            v.body_fall = 1.;
            v.noise = 0.;
            v.noise_fall = 0.;
            v.pitch = 0.;
            v.pitch_fall = 0.;
        }
        if let Some(shape) = color::shape(self.kit, id, d.variant) {
            v.colored = id != 2;
            v.banked = true;
            v.seed = 0x5831a17d ^ (id as u32 * 0x1000193) ^ self.kit as u32;
            let raw_tune = ((d.tune + 6.) / 12. * 127.).round();
            let speed = if self.kit == 1 && id >= 10 {
                (256. * 2.0_f32.powf((raw_tune - 64.) / 144.)).floor() / 256.
            } else {
                1.
            };
            // Pitch the procedurally generated source through a two-frame interpolator.
            let pitched = self.kit == 1 && id >= 10;
            let generator_rate = if pitched { 44100. } else { sr };
            let generator_speed = if pitched { 1. } else { speed };
            v.source_step = if pitched {
                speed as f64 * 44100. / sr as f64
            } else {
                0.
            };
            for i in 0..8 {
                let [hz, q, amp, tau] = shape.bands[i];
                v.banks[i] = Band::new(hz * generator_speed, q, generator_rate);
                v.color_gain[i] = amp * (generator_rate / (44100. * generator_speed)).sqrt();
                v.color_fall[i] = (-generator_speed / (tau * generator_rate)).exp();
            }
            v.color_hp_alpha = lp(shape.highpass_hz * generator_speed, generator_rate);
            // Start a noise excitation in its stationary state; the envelope supplies the attack.
            for _ in 0..(generator_rate * 0.006).round() as usize {
                colored_noise(&mut v, false);
            }
            v.freq = [
                0.,
                0.,
                shape.attack / generator_speed,
                0.,
                shape.hold / generator_speed,
                shape.end / generator_speed,
            ];
            v.env = 1.;
            v.noise = 0.;
            v.noise_fall = 0.;
            v.body_fall = 1.;
            v.fall = 1.;
            v.noise_cut = 0.;
            if id == 2 {
                v.noise = 1.;
                v.noise_fall = if self.kit == 0 {
                    1.
                } else {
                    0.998_f32.powf(44100. / (2. * sr))
                };
                v.hold = if self.kit == 0 {
                    0
                } else {
                    ((d.tone.clamp(0., 1.) * 127. + 30.) * 64. * sr / 44100.).round() as u32
                };
            } else if self.kit == 0 && id == 9 {
                v.noise_cut = tone_alpha(d.tone, sr);
                v.tone_setting = d.tone;
                v.fall = drum_decay(d.decay, sr, 0);
            } else if self.kit == 0 && id == 10 {
                let raw = (((d.decay - 0.3) / 1.7).clamp(0., 1.) * 127.).round();
                v.hold = (4. * raw * 64. * sr / 44100.).round() as u32;
                v.fall = 0.998_f32.powf(44100. / (2. * sr));
            } else if self.kit == 1 && (id == 8 || id == 9) {
                v.fall = drum_decay(d.decay, sr, if id == 8 { 8 } else { 1 });
            }
        }
        self.voices[id] = v;
    }
    pub fn sample(&mut self, kit: &DrumKit) -> f32 {
        if kit.voices.len() != 12 {
            return 0.;
        }
        let sr = self.sr;
        let mut out = 0.;
        for id in 1..12 {
            let d = &kit.voices[id];
            if let Some(v) = &mut self.samples[id] {
                if let Some(sound) = sample_voice(v, d, self.kit, id, sr) { out += sound; }
                else { self.samples[id] = None; }
                continue;
            }
            let v = &mut self.voices[id];
            if v.env < 1e-6 && v.body < 1e-6 && v.noise < 1e-6 {
                continue;
            }
            let t = v.age as f32 / sr;
            if self.kit == 0 && id == 9 && d.tone != v.tone_setting {
                v.noise_cut = tone_alpha(d.tone, sr);
                v.tone_setting = d.tone;
            }
            if self.kit == 0 && id == 1 && d.tone != v.tone_setting {
                v.freq[5] = tone_alpha(d.tone, sr);
                v.tone_setting = d.tone;
            }
            let texture = if v.banked && !v.colored {
                colored_signal(v, t, self.kit, id)
            } else {
                0.
            };
            let sound = if v.mode_count > 0 {
                let body = membrane_sample(v, sr);
                let snappy = d.snappy.clamp(0., 1.);
                body + if id == 2 {
                    texture
                        * v.noise
                        * if self.kit == 0 {
                            snappy * snappy
                        } else {
                            1.27 * snappy
                        }
                } else {
                    0.
                }
            } else if v.colored {
                let signal = if v.source_step > 0. {
                    pitched_color(v, self.kit, id)
                } else {
                    colored_signal(v, t, self.kit, id)
                } * v.env;
                if self.kit == 0 && id == 9 {
                    v.tone_state = signal + (1. - v.noise_cut) * v.tone_state;
                    v.tone_state
                } else {
                    signal
                }
            } else if id == 1 {
                kick_sample(v, sr)
            } else if self.kit == 0 && id == 8 {
                if t >= v.mode_end {
                    v.env = 0.;
                    v.body = 0.;
                    v.noise = 0.;
                }
                let a = pulse(&mut v.phase[0], v.freq[0] / sr, 0.1);
                let b = pulse(&mut v.phase[1], v.freq[1] / sr, 0.5);
                v.band.process(1.02 * a + 0.8 * b) * (v.body + v.noise)
            } else {
                0.
            };
            out += sound * v.velocity * voice_gain(self.kit, id, d.level);
            advance(v);
        }
        if let Some(v) = &mut self.bank_kick_tail {
            if let Some(sound) = sample_voice(v, &kit.voices[1], self.kit, 1, sr) { out += sound; }
            else { self.bank_kick_tail = None; }
        }
        if let Some(v) = &mut self.kick_tail {
            if v.env > 1e-6 {
                out +=
                    kick_sample(v, sr) * v.velocity * voice_gain(self.kit, 1, kit.voices[1].level);
                advance(v);
            }
        }
        out
    }
}
// Gain constants and operand order follow the OpenRB-338 GPL-3.0 reference.
fn bank_trigger_gain(kit: usize, id: usize, level: f32, velocity: f32, pole: Option<f32>) -> f32 {
    let mut raw = (level.clamp(0., 1.) * 127.).round() as u32;
    if kit == 0 && id == 10 { raw /= 2; }
    if kit == 0 && id == 11 { raw = 2 * raw / 3; }
    let l = raw as f64;
    let k = f32::from_bits(0x3c01_0204) as f64;
    let velocity = velocity as f64;
    let gain = if kit == 0 {
        if (3..=5).contains(&id) { (((l * k) * l) * k) * velocity }
        else if id == 2 { (((l * k) * l) * f32::from_bits(0x3ccc_cccd) as f64) * velocity }
        else { (((k * l) * l) * f32::from_bits(0x3ba3_d70a) as f64) * velocity }
    } else if id <= 2 {
        ((l * (l * k)) * k) * velocity
    } else {
        let c = [0., 0., 0., 1.2_f32, 1.1, 1.4, 1.1, 2.3, 0.8, 1.1, 0.6, 0.6][id] as f64;
        if (3..=5).contains(&id) || id >= 10 { ((l * (l * f32::from_bits(0x3882_0610) as f64)) * c) * velocity }
        else { (((f32::from_bits(0x3882_0610) as f64 * l) * l) * c) * velocity }
    };
    (if id == 1 { gain * (1. - pole.unwrap_or(0.) as f64) } else { gain }) as f32
}
fn sample_voice(v: &mut SampleVoice, d: &rb_model::Drum, _kit: usize, _id: usize, sr: f32) -> Option<f32> {
    if v.bank_ended { return None; }
    let i = v.position as usize;
    let len = v.noise.as_ref().map_or(v.sample.pcm.len(), |n| n.pcm.len().min(v.sample.pcm.len()));
    let end = if v.bank_pitch.is_some() { (len + 63) / 64 * 64 } else { len };
    if !v.bank_pitched && i >= end { return None; }
    let read = |s: &rb_model::samples::CustomSample| {
        let a = s.pcm.get(i).copied().unwrap_or(0) as i32;
        let b = s.pcm.get(i + 1).copied().unwrap_or(0) as i32;
        if v.bank_pitched && sr == 44100. && s.sample_rate == 44100 {
            let frac = ((v.position - i as f64) * 256.) as i32;
            (a + ((frac * (b - a)) >> 8)) as i16 as f64
        } else { a as f64 + (b - a) as f64 * (v.position - i as f64) }
    };
    let out = if v.bank_pitch.is_some() {
        let value = read(&v.sample);
        let result = if let Some(noise) = &v.noise {
            (value + read(noise) * v.bank_env) * v.bank_gain as f64
        } else if let Some(original_pole) = v.filter_pole {
            let pole = (original_pole as f64).powf(44100. / sr as f64);
            let adaptation = (1. - pole) / (1. - original_pole as f64);
            v.filter_state = v.filter_state * pole + value * v.bank_env * adaptation;
            v.filter_state
        } else { value * v.bank_env };
        let previous = v.bank_clock;
        v.bank_clock += 44100. / sr as f64;
        let pairs = ((v.bank_clock - v.bank_hold).max(0.) / 2.).floor() - ((previous - v.bank_hold).max(0.) / 2.).floor();
        v.bank_env *= (v.bank_fall as f64).powf(pairs);
        if (v.bank_clock / 64.).floor() > (previous / 64.).floor() {
            if !v.bank_pitched {
                let gain = if v.noise.is_some() { v.bank_gain as f64 } else { v.bank_env };
                if gain < f32::from_bits(0x3300_d959) as f64 { v.bank_ended = true; }
            } else if v.position + v.bank_pitch.unwrap() * v.sample.sample_rate as f64 / sr as f64 > len as f64 { v.bank_ended = true; }
            v.bank_env = v.bank_env as f32 as f64;
            v.filter_state = v.filter_state as f32 as f64;
        }
        result as f32
    } else {
        let value = read(&v.sample) as f32 / 32768.;
        let result = value * v.velocity * d.level.clamp(0., 1.5).powi(2) * v.env * 2.18;
        let decay = d.decay.clamp(0.05, 2.);
        v.env *= v.choke * (-((1. / decay - 1.).max(0.)) / sr).exp();
        result
    };
    let step = v.bank_pitch.unwrap_or_else(|| 2f64.powf(d.tune.clamp(-24., 24.) as f64 / 12.));
    v.position += v.sample.sample_rate as f64 / sr as f64 * step;
    Some(out)
}
fn advance(v: &mut Voice) {
    if v.age >= v.hold {
        v.env *= v.fall;
        v.noise *= v.noise_fall;
    }
    v.body *= v.body_fall;
    v.pitch *= v.pitch_fall;
    v.age = v.age.saturating_add(1);
}
fn kick_sample(v: &mut Voice, sr: f32) -> f32 {
    let t = v.age as f32 / sr;
    if t >= v.mode_end {
        v.env = 0.;
        v.body = 0.;
        v.noise = 0.;
        v.tone_state = 0.;
        return 0.;
    }
    let mut wave = 0.;
    for i in 0..3 {
        wave += (TAU * ((i + 1) as f32 * v.phase[0] + v.modes[i].phase)).sin() * v.modes[i].amp;
    }
    v.phase[0] = (v.phase[0] + v.freq[0] + v.freq[1] * v.pitch + v.freq[2] * v.noise).fract();
    v.body = (-v.freq[3] * t - v.freq[4] * t * t).exp();
    wave *= v.body;
    for i in 0..2 {
        wave += v.mode_pulse[i];
        v.mode_pulse[i] *= v.mode_pulse_fall[i];
    }
    v.tone_state += v.freq[5] * (wave * v.env - v.tone_state);
    v.tone_state
}
fn voice_gain(kit: usize, id: usize, level: f32) -> f32 {
    let mut level = level.clamp(0., 1.5);
    if kit == 0 && (id == 10 || id == 11) {
        let raw = (level * 127.).round();
        level = if id == 10 {
            (raw / 2.).floor() / 127.
        } else {
            (2. * raw / 3.).floor() / 127.
        };
    }
    let coefficient = if kit == 0 {
        [
            0., 0.635, 3.175, 1., 1., 1., 0.635, 0.635, 0.635, 0.635, 0.635, 0.635,
        ][id]
    } else {
        [
            0., 1., 1., 1.1999976, 1.0999978, 1.3999972, 1.0999978, 2.2999954, 0.7999984,
            1.0999978, 0.5999988, 0.5999988,
        ][id]
    };
    level * level * coefficient
}
fn lp(hz: f32, sr: f32) -> f32 {
    1. - (-TAU * hz.min(sr * 0.45) / sr).exp()
}
fn tone_alpha(tone: f32, sr: f32) -> f32 {
    let index = ((tone.clamp(0., 1.) * 8.) as usize).min(7);
    let pole = [0.9939_f32, 0.9757, 0.906, 0.822, 0.675, 0.456, 0.208, 0.001][index];
    1. - pole.powf(44100. / sr)
}
// Generated control curves, not waveform data. A multiplier acts once per sample pair.
fn bank_decay(value: f32, kit: usize) -> f32 {
    let index = ((((value - 0.3) / 1.7).clamp(0., 1.) * 16.) as usize).min(15);
    if kit == 0 {
        [0.999, 0.9993, 0.9995, 0.99955, 0.9996089, 0.9996524, 0.9996958, 0.9997393, 0.9997828, 0.99982, 0.99986, 0.999913, 0.99993, 0.999956, 0.99997, 1.][index]
    } else {
        let pole = (1.0_f64 - (15 - index) as f64 * 0.000025) as f32;
        (pole as f64).powi(kit as i32) as f32
    }
}
fn drum_decay(value: f32, sr: f32, kit: usize) -> f32 {
    let index = ((((value - 0.3) / 1.7).clamp(0., 1.) * 16.) as usize).min(15);
    let pole = if kit == 0 {
        [
            0.999_f32, 0.9993, 0.9995, 0.99955, 0.9996089, 0.9996524, 0.9996958, 0.9997393,
            0.9997828, 0.99982, 0.99986, 0.999913, 0.99993, 0.999956, 0.99997, 1.,
        ][index]
    } else {
        (1. - (15 - index) as f32 * 0.000025).powi(kit as i32)
    };
    pole.powf(44100. / (2. * sr))
}
fn pulse(phase: &mut f32, dt: f32, duty: f32) -> f32 {
    let dt = dt.clamp(0., 0.45);
    let p = *phase;
    *phase = (p + dt).fract();
    let mut y = if p < duty { 1. } else { -1. };
    y += blep(p, dt);
    y -= blep((p + 1. - duty).fract(), dt);
    y
}
fn blep(t: f32, dt: f32) -> f32 {
    if dt <= 0. {
        0.
    } else if t < dt {
        let x = t / dt;
        x + x - x * x - 1.
    } else if t > 1. - dt {
        let x = (t - 1.) / dt;
        x * x + x + x + 1.
    } else {
        0.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rb_model::Song;
    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|x| x * x).sum::<f32>() / x.len() as f32).sqrt()
    }
    #[test]
    fn voices_are_distinct_finite_and_mutable() {
        for sr in [44100., 48000., 96000.] {
            for k in 0..2 {
                let mut kit = Song::default().drums[k].clone();
                let mut signals = Vec::new();
                for id in 1..12 {
                    let mut d = DrumSynth::new(sr, k);
                    d.trigger(id, 1., &kit);
                    let v: Vec<f32> = (0..(sr * 0.06) as usize).map(|_| d.sample(&kit)).collect();
                    assert!(v.iter().all(|x| x.is_finite()));
                    assert!(rms(&v) > 0.00001, "{k}:{id}");
                    signals.push(v);
                    kit.voices[id].level = 0.;
                    assert_eq!(d.sample(&kit), 0.);
                    kit.voices[id].level = 0.8;
                }
                for i in 0..10 {
                    assert_ne!(signals[i], signals[i + 1]);
                }
            }
        }
    }
    #[test]
    fn choke_priority() {
        for k in 0..2 {
            let kit = Song::default().drums[k].clone();
            let mut d = DrumSynth::new(48000., k);
            let (ch, oh) = if k == 0 { (11, 10) } else { (8, 9) };
            d.trigger(oh, 1., &kit);
            for _ in 0..100 {
                d.sample(&kit);
            }
            d.trigger(ch, 1., &kit);
            assert!(d.voices[oh].body > 0. && d.voices[oh].hold == 0);
            let short_fall = d.voices[oh].fall;
            d.trigger(oh, 1., &kit);
            assert!(d.voices[ch].body > 0. && d.voices[oh].body > 0.);
            if k == 0 {
                assert!(d.voices[oh].hold > 0);
            } else {
                assert!(d.voices[oh].fall > short_fall);
            }
        }
    }
    #[test]
    fn tuning_anchors() {
        let kit = Song::default().drums[0].clone();
        let mut d = DrumSynth::new(48000., 0);
        d.trigger(2, 1., &kit);
        assert!((d.voices[2].modes[0].step * 48000. - 178.).abs() < 2.);
        assert!((d.voices[2].modes[1].step * 48000. - 332.).abs() < 3.);
        d.trigger(8, 1., &kit);
        assert_eq!(d.voices[8].freq[0], 537.279);
        assert_eq!(d.voices[8].freq[1], 809.69);
    }
    #[test]
    fn kick_tuning_changes_sweep_and_preserves_final_pitch() {
        let mut kit = Song::default().drums[1].clone();
        let mut starts = Vec::new();
        for tune in [-6., -2., 2., 6.] {
            kit.voices[1].tune = tune;
            let mut d = DrumSynth::new(44100., 1);
            d.trigger(1, 1., &kit);
            starts.push(d.voices[1].pitch_fall);
            for _ in 0..22050 {
                d.sample(&kit);
            }
            let v = &d.voices[1];
            let frequency = (v.freq[0] + v.freq[1] * v.pitch + v.freq[2] * v.noise) * 44100.;
            assert!((53. ..55.).contains(&frequency), "Tune {tune}: {frequency}");
        }
        assert!(starts.windows(2).all(|p| p[1] > p[0]));
    }
    #[test]
    fn kick_retrigger_retains_fading_tail_and_clear_silences_it() {
        for k in 0..2 {
            let kit = Song::default().drums[k].clone();
            let mut d = DrumSynth::new(44100., k);
            d.trigger(1, 1., &kit);
            for _ in 0..4410 {
                d.sample(&kit);
            }
            let age = d.voices[1].age;
            d.trigger(1, 1., &kit);
            let tail = d.kick_tail.as_ref().unwrap();
            assert_eq!(tail.age, age);
            assert!(tail.env > 0. && tail.fall < d.voices[1].fall);
            assert!(d.sample(&kit).is_finite());
            d.clear();
            assert!(d.kick_tail.is_none());
            assert!((0..128).all(|_| d.sample(&kit) == 0.));
        }
    }
    #[test]
    fn selectors_and_control_extremes_are_finite_and_end() {
        for sr in [8000., 44100., 96000.] {
            for k in 0..2 {
                let mut kit = Song::default().drums[k].clone();
                for id in 1..12 {
                    for high in [false, true] {
                        let v = &mut kit.voices[id];
                        v.variant = high;
                        v.tune = if high { 6. } else { -6. };
                        v.tone = if high { 1. } else { 0. };
                        v.decay = if high { 2. } else { 0.3 };
                        let mut d = DrumSynth::new(sr, k);
                        d.trigger(id, 1., &kit);
                        assert!(
                            (0..(sr * 0.12) as usize).all(|_| d.sample(&kit).is_finite()),
                            "{sr}:{k}:{id}:{high}"
                        );
                        if k == 0 && id == 8 {
                            for _ in 0..(sr * 1.1) as usize {
                                d.sample(&kit);
                            }
                            assert_eq!(d.voices[id].env, 0.);
                        }
                    }
                }
            }
        }
    }
}
#[cfg(test)]
mod render_measurements {
    use super::*;
    use rb_model::Song;
    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|x| x * x).sum::<f32>() / x.len() as f32).sqrt()
    }
    fn spectral(x: &[f32], freq: f32, sr: f32) -> f32 {
        let (a, b) = x.iter().enumerate().fold((0., 0.), |(a, b), (i, v)| {
            let p = TAU * freq * i as f32 / sr;
            (a + v * p.cos(), b + v * p.sin())
        });
        (a * a + b * b).sqrt()
    }
    #[test]
    fn rendered_snare_pitch_and_decay() {
        let sr = 48000.;
        let mut kit = Song::default().drums[0].clone();
        kit.voices[2].snappy = 0.;
        let mut d = DrumSynth::new(sr, 0);
        d.trigger(2, 1., &kit);
        let x: Vec<_> = (0..4800).map(|_| d.sample(&kit)).collect();
        let segment = &x[..2880];
        assert!(spectral(segment, 178., sr) > spectral(segment, 100., sr) * 3.);
        assert!(spectral(segment, 332., sr) > spectral(segment, 800., sr) * 3.);
        let ratio = rms(&x[3360..3840]) / rms(&x[480..960]);
        assert!((0.075..0.14).contains(&ratio), "decay ratio {ratio}");
    }
    #[test]
    fn snare_tone_changes_resonance_balance_without_retuning() {
        let sr = 48000.;
        let mut ratios = Vec::new();
        for tone in [-6., 6.] {
            let mut kit = Song::default().drums[0].clone();
            kit.voices[2].snappy = 0.;
            kit.voices[2].tune = tone;
            let mut d = DrumSynth::new(sr, 0);
            d.trigger(2, 1., &kit);
            let x: Vec<_> = (0..4800).map(|_| d.sample(&kit)).collect();
            let low = spectral(&x, 178., sr);
            let high = spectral(&x, 332., sr);
            assert!(low > spectral(&x, 120., sr) * 2.);
            assert!(high > spectral(&x, 500., sr) * 2.);
            ratios.push(high / low);
        }
        assert!(ratios[1] > ratios[0] * 3.);
    }
    #[test]
    fn cymbal_tone_changes_sounding_tail() {
        let mut low = Song::default().drums[0].clone();
        low.voices[9].tone = 0.;
        let mut high = low.clone();
        let mut a = DrumSynth::new(48000., 0);
        let mut b = DrumSynth::new(48000., 0);
        a.trigger(9, 1., &low);
        b.trigger(9, 1., &low);
        for _ in 0..2400 {
            a.sample(&low);
            b.sample(&low);
        }
        high.voices[9].tone = 1.;
        let x: Vec<_> = (0..4800).map(|_| a.sample(&low)).collect();
        let y: Vec<_> = (0..4800).map(|_| b.sample(&high)).collect();
        let (rx, ry) = (rms(&x), rms(&y));
        let difference = x
            .iter()
            .zip(&y)
            .map(|(x, y)| (x / rx - y / ry).powi(2))
            .sum::<f32>()
            / x.len() as f32;
        assert!(difference > 0.02, "normalized tone difference {difference}");
    }
    #[test]
    fn rimshot_high_resonance_decays_before_low_body() {
        let kit = Song::default().drums[1].clone();
        let mut d = DrumSynth::new(48000., 1);
        d.trigger(6, 1., &kit);
        let x: Vec<_> = (0..2400).map(|_| d.sample(&kit)).collect();
        let ratio = |x: &[f32]| spectral(x, 1037., 48000.) / spectral(x, 223., 48000.);
        assert!(ratio(&x[960..1440]) < ratio(&x[..480]) * 0.3);
    }
    #[test]
    fn rendered_hat_choke() {
        let sr = 48000.;
        let kit = Song::default().drums[0].clone();
        let mut open = DrumSynth::new(sr, 0);
        let mut choked = DrumSynth::new(sr, 0);
        open.trigger(10, 1., &kit);
        choked.trigger(10, 1., &kit);
        for _ in 0..2400 {
            open.sample(&kit);
            choked.sample(&kit);
        }
        choked.trigger(11, 1., &kit);
        let a: Vec<_> = (0..4800).map(|_| open.sample(&kit)).collect();
        let b: Vec<_> = (0..4800).map(|_| choked.sample(&kit)).collect();
        assert!(rms(&b[2400..]) < rms(&a[2400..]) * 0.15);
    }
}

#[cfg(test)]
mod custom_sample_tests {
    use super::*;
    #[test]
    fn sample_override_tune_retrigger_and_choke() {
        let mut kit = rb_model::Song::default().drums[0].clone();
        for d in &mut kit.voices { d.level=1.; d.tune=0.; d.decay=1.; }
        let sample=std::sync::Arc::new(rb_model::samples::CustomSample {name:"test".into(), sample_rate:16000, pcm:vec![16384;8000]});
        let mut d=DrumSynth::new(8000.,0);
        d.trigger_sample(10,1.,&kit,Some(&sample));
        assert!((d.sample(&kit)-0.5 * 2.18).abs()<1e-5);
        for _ in 0..100 { d.sample(&kit); }
        assert_eq!(d.samples[10].as_ref().unwrap().position,202.);
        let before=d.samples[10].as_ref().unwrap().position;
        kit.voices[10].tune=12.; d.sample(&kit);
        assert_eq!(d.samples[10].as_ref().unwrap().position-before,4.);
        d.trigger_sample(10,0.5,&kit,Some(&sample));
        assert_eq!(d.samples[10].as_ref().unwrap().position,0.);
        d.trigger_sample(11,1.,&kit,Some(&sample));
        assert!(d.samples[10].as_ref().unwrap().choke<1.);
        d.clear(); assert!(d.samples.iter().all(Option::is_none));
        kit.voices[1].level=0.;
        d.trigger_sample(1,1.,&kit,Some(&sample));
        assert_eq!(d.sample(&kit),0.);
        kit.voices[1].level=1.;
        d.trigger_sample(1,1.,&kit,None);
        assert!(d.samples[1].is_none());
        assert!(d.voices[1].env>0.);

    }
    #[test]
    fn bank_selectors_pitch_snare_and_manual_override() {
        let mut kit = rb_model::Song::default().drums[0].clone();
        let bank=std::sync::Arc::new(rb_model::samples::SampleBank {
            name:"marker".into(), samples:(0..46).map(|id| Some(std::sync::Arc::new(rb_model::samples::CustomSample {
                name:id.to_string(), sample_rate:44100, pcm:vec![id as i16 * 100;16],
            }))).collect(),
        });
        let mut a=DrumSynth::new(44100.,0);
        for (id,variant,expected) in [(3,false,18),(3,true,17),(4,false,21),(5,true,15),(6,false,23),(6,true,12),(7,false,13),(7,true,19),(8,false,10),(9,false,14),(10,false,22),(11,false,11)] {
            kit.voices[id].variant=variant;
            a.trigger_bank(id,1.,&kit,None,Some(&bank));
            assert_eq!(a.samples[id].as_ref().unwrap().sample.name,expected.to_string());
        }
        kit.voices[2].tone=0.;
        a.trigger_bank(2,1.,&kit,None,Some(&bank));
        assert_eq!(a.samples[2].as_ref().unwrap().sample.name,"1");
        assert_eq!(a.samples[2].as_ref().unwrap().noise.as_ref().unwrap().name,"7");
        let mut b=DrumSynth::new(44100.,1);
        kit.voices[1].tune=6.;
        b.trigger_bank(1,1.,&kit,None,Some(&bank));
        assert_eq!(b.samples[1].as_ref().unwrap().sample.name,"28");
        kit.voices[2].tune=0.;
        b.trigger_bank(2,1.,&kit,None,Some(&bank));
        assert_eq!(b.samples[2].as_ref().unwrap().sample.name,"31");
        assert_eq!(b.samples[2].as_ref().unwrap().noise.as_ref().unwrap().name,"35");
        for id in 3..=11 {
            b.trigger_bank(id,1.,&kit,None,Some(&bank));
            assert_eq!(b.samples[id].as_ref().unwrap().sample.name,(34+id).to_string());
        }
        for (raw,expected) in [(64,256),(66,258),(90,290)] {
            kit.voices[10].tune=raw as f32/127.*12.-6.;
            b.trigger_bank(10,1.,&kit,None,Some(&bank));
            assert_eq!((b.samples[10].as_ref().unwrap().bank_pitch.unwrap()*256.) as usize,expected);
        }
        let manual=std::sync::Arc::new(rb_model::samples::CustomSample {name:"manual".into(),sample_rate:8000,pcm:vec![1]});
        b.trigger_bank(10,1.,&kit,Some(&manual),Some(&bank));
        assert_eq!(b.samples[10].as_ref().unwrap().sample.name,"manual");
        assert!(b.samples[10].as_ref().unwrap().bank_pitch.is_none());
        let empty=std::sync::Arc::new(rb_model::samples::SampleBank {name:"empty".into(),samples:vec![None;46]});
        b.trigger_bank(1,1.,&kit,None,Some(&empty));
        assert!(b.samples[1].is_none());
        assert!(b.voices[1].env>0.);
    }

    #[test]
    fn imported_808_selector_flags_select_the_original_resources() {
        let mut raw = include_bytes!("../../../songs/rbs-examples/Peff-3pm.rbs").to_vec();
        let offset = raw.windows(4).position(|x|x==b"808 ").unwrap()+8;
        let bank=std::sync::Arc::new(rb_model::samples::SampleBank {name:"markers".into(), samples:(0..46).map(|i|Some(std::sync::Arc::new(rb_model::samples::CustomSample {name:i.to_string(),sample_rate:44100,pcm:vec![0;64]}))).collect()});
        for (control, voice, low, high) in [(11,3,17,18),(14,4,20,21),(17,5,15,16),(19,6,12,23),(21,7,19,13)] {
            for (raw_value,expected) in [(0,low),(1,high)] {
                raw[offset+control]=raw_value;
                let song=rb_model::rbs::import(&raw).unwrap();
                let mut d=DrumSynth::new(44100.,0);
                d.trigger_bank(voice,1.,&song.drums[0],None,Some(&bank));
                assert_eq!(d.samples[voice].as_ref().unwrap().sample.name,expected.to_string());
            }
        }
    }
    #[test]
    fn bank_gain_filter_decay_snapshot_and_kick_tail() {
        let bank = std::sync::Arc::new(rb_model::samples::SampleBank {
            name: "constant".into(), samples: (0..46).map(|_| Some(std::sync::Arc::new(
                rb_model::samples::CustomSample { name: "constant".into(), sample_rate: 44100, pcm: vec![16384; 30000] }
            ))).collect(),
        });
        for k in 0..2 {
            let mut kit = rb_model::Song::default().drums[k].clone();
            kit.voices[1].level = 1.; kit.voices[1].decay = 0.3;
            let mut a = DrumSynth::new(44100., k);
            let mut b = DrumSynth::new(44100., k);
            a.trigger_bank(1, 1., &kit, None, Some(&bank));
            b.trigger_bank(1, 1., &kit, None, Some(&bank));
            let first = a.sample(&kit);
            assert!(first > 0. && first < 0.5 * voice_gain(k, 1, 1.));
            assert_eq!(first, b.sample(&kit));
            let mut changed = kit.clone(); changed.voices[1].level = 0.; changed.voices[1].decay = 2.; changed.voices[1].tone = 1.;
            for _ in 0..256 { assert_eq!(a.sample(&kit), b.sample(&changed)); }
            assert!(a.samples[1].as_ref().unwrap().bank_env < 1.);
            a.trigger_bank(1, 1., &kit, None, Some(&bank));
            assert!(a.bank_kick_tail.is_some());
            let before = a.bank_kick_tail.as_ref().unwrap().bank_env;
            for _ in 0..20 { a.sample(&kit); }
            assert!(a.bank_kick_tail.as_ref().unwrap().bank_env < before);
        }
        let mut kit = rb_model::Song::default().drums[0].clone();
        kit.voices[9].level = 1.; kit.voices[9].tone = 0.;
        let mut cy = DrumSynth::new(44100., 0);
        cy.trigger_bank(9, 1., &kit, None, Some(&bank));
        assert!((cy.sample(&kit) - 0.5 * voice_gain(0, 9, 1.)).abs() < 1e-6);
    }
    #[test]
    fn bank_filter_keeps_double_precision_and_pitched_uses_integer_interpolation() {
        let mut bank = rb_model::samples::SampleBank { name: "numerics".into(), samples: vec![None; 46] };
        bank.samples[14] = Some(std::sync::Arc::new(rb_model::samples::CustomSample { name: "filter".into(), sample_rate: 44100, pcm: vec![12345; 1024] }));
        bank.samples[18] = Some(std::sync::Arc::new(rb_model::samples::CustomSample { name: "pitch".into(), sample_rate: 44100, pcm: vec![0, 1, 1, 1] }));
        let bank = std::sync::Arc::new(bank);
        let mut kit = rb_model::Song::default().drums[0].clone();
        kit.voices[9].tone = 0.; kit.voices[9].decay = 2.;
        let mut d = DrumSynth::new(44100., 0);
        d.trigger_bank(9, 1., &kit, None, Some(&bank));
        let v = d.samples[9].as_ref().unwrap();
        let mut expected = 0_f64;
        let gain = v.bank_gain as f64;
        let pole = v.filter_pole.unwrap() as f64;
        for _ in 0..63 {
            expected = expected * pole + 12345. * gain;
            assert_eq!(d.sample(&kit), expected as f32);
        }
        d.clear(); kit.voices[3].variant = false;
        d.trigger_bank(3, 1., &kit, None, Some(&bank));
        d.samples[3].as_mut().unwrap().position = 0.5;
        assert_eq!(d.sample(&kit), 0.);
    }
    #[test]
    fn bank_cymbal_filter_preserves_gain_across_sample_rates() {
        let bank = std::sync::Arc::new(rb_model::samples::SampleBank {
            name: "constant".into(), samples: (0..46).map(|_| Some(std::sync::Arc::new(
                rb_model::samples::CustomSample { name: "constant".into(), sample_rate: 44100, pcm: vec![8192; 30000] }
            ))).collect(),
        });
        let mut kit = rb_model::Song::default().drums[0].clone();
        kit.voices[9].tone = 0.; kit.voices[9].decay = 2.; kit.voices[9].level = 1.;
        let mut a = DrumSynth::new(44100., 0);
        let mut b = DrumSynth::new(48000., 0);
        a.trigger_bank(9, 1., &kit, None, Some(&bank));
        b.trigger_bank(9, 1., &kit, None, Some(&bank));
        let x = (0..4410).map(|_| a.sample(&kit)).last().unwrap();
        let y = (0..4800).map(|_| b.sample(&kit)).last().unwrap();
        assert!((x - y).abs() < x.abs() * 0.0001, "{x} {y}");
    }
    #[test]
    fn bank_dual_noise_decays_and_hat_chokes() {
        let bank = std::sync::Arc::new(rb_model::samples::SampleBank {
            name: "constant".into(), samples: (0..46).map(|_| Some(std::sync::Arc::new(
                rb_model::samples::CustomSample { name: "constant".into(), sample_rate: 44100, pcm: vec![8192; 30000] }
            ))).collect(),
        });
        let mut kit = rb_model::Song::default().drums[1].clone();
        kit.voices[2].tone = 0.; kit.voices[2].snappy = 1.; kit.voices[2].level = 1.;
        let mut d = DrumSynth::new(44100., 1);
        d.trigger_bank(2, 1., &kit, None, Some(&bank));
        let initial = d.sample(&kit);
        for _ in 0..4000 { d.sample(&kit); }
        let later = d.sample(&kit);
        assert!(later < initial && later > 0.25 * voice_gain(1, 2, 1.));
        d.trigger_bank(9, 1., &kit, None, Some(&bank));
        d.trigger_bank(8, 1., &kit, None, Some(&bank));
        assert_eq!(d.samples[9].as_ref().unwrap().bank_fall, 0.996);
        let mut high_rate = DrumSynth::new(48000., 1);
        high_rate.trigger_bank(9, 1., &kit, None, Some(&bank));
        high_rate.trigger_bank(8, 1., &kit, None, Some(&bank));
        for _ in 0..480 { high_rate.sample(&kit); }
        let open = high_rate.samples[9].as_ref().unwrap();
        assert!((open.bank_clock - 441.).abs() < 0.001);
        assert!((open.bank_env / open.bank_gain as f64 - (0.996_f32 as f64).powi(220)).abs() < 0.00001);
    }
    #[test]
    fn mixed_bank_samples_finish_after_shorter_source() {
        for (tone_len,noise_len) in [(4,8),(8,4)] {
            let mut kit = rb_model::Song::default().drums[0].clone();
            kit.voices[2].tone=0.; kit.voices[2].snappy=1.;
            kit.voices[2].level=1.; kit.voices[2].decay=1.;
            let mut bank=rb_model::samples::SampleBank { name:"test".into(), samples:vec![None;46] };
            for (id,len) in [(1,tone_len),(7,noise_len)] {
                bank.samples[id]=Some(std::sync::Arc::new(rb_model::samples::CustomSample {
                    name:id.to_string(),sample_rate:44100,pcm:vec![8192;len],
                }));
            }
            let bank=std::sync::Arc::new(bank);
            let mut d=DrumSynth::new(44100.,0);
            d.trigger_bank(2,1.,&kit,None,Some(&bank));
            for i in 0..tone_len.min(noise_len) { assert!(d.sample(&kit)>0., "frame {i}, {tone_len}/{noise_len}"); }
            for _ in tone_len.min(noise_len)..64 { d.sample(&kit); }
            assert_eq!(d.sample(&kit),0.);
        }
    }

}
