//! Generated percussion: reduced sound models, not recordings or circuit solvers.
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

pub struct DrumSynth {
    sr: f32,
    kit: usize,
    voices: [Voice; 12],
    kick_tail: Option<Voice>,
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
        }
    }
    pub fn clear(&mut self) {
        self.voices = [Voice::default(); 12];
        self.kick_tail = None;
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
    // Preserve the reference bus excitation before the rack's existing 2.18 normalization.
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
    level * level * coefficient / 2.18
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
