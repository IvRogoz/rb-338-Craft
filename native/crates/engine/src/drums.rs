//! Generated percussion: reduced sound models, not recordings or circuit solvers.
use rb_model::DrumKit;
use std::f32::consts::TAU;

#[derive(Clone, Copy)]
struct Voice {
    env: f32,
    body: f32,
    noise: f32,
    age: u32,
    phase: [f32; 6],
    freq: [f32; 6],
    fall: f32,
    body_fall: f32,
    noise_fall: f32,
    noise_cut: f32,
    noise_low: f32,
    noise_mix: f32,
    pitch: f32,
    pitch_fall: f32,
    low: f32,
    high: f32,
    tone_state: f32,
    tone_setting: f32,
    band: Band,
    modes: [Mode; 32],
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
            phase: [0.; 6],
            freq: [0.; 6],
            fall: 0.,
            body_fall: 0.,
            noise_fall: 0.,
            noise_cut: 0.,
            noise_low: 0.,
            noise_mix: 0.,
            pitch: 0.,
            pitch_fall: 0.,
            low: 0.,
            high: 0.,
            tone_state: 0.,
            tone_setting: f32::NAN,
            band: Band::default(),
            modes: [Mode::default(); 32],
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

pub struct DrumSynth {
    sr: f32,
    kit: usize,
    voices: [Voice; 12],
    metal: [f32; 6],
    rng: u32,
    closed_now: bool,
    metal_band: Band,
    noise_alpha: [f32; 2],
    tone_alpha: [f32; 65],
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
            metal: [0.; 6],
            rng: 0x5831a17d,
            closed_now: false,
            metal_band: Band::new(7100., 1.5, sr),
            noise_alpha: [lp(1500., sr), lp(6000., sr)],
            tone_alpha: std::array::from_fn(|i| lp(180. + i as f32 / 64. * 4500., sr)),
        }
    }
    pub fn clear(&mut self) {
        self.voices = [Voice::default(); 12];
        self.metal = [0.; 6];
        self.metal_band = Band::new(7100., 1.5, self.sr);
        self.closed_now = false;
        self.rng = 0x5831a17d;
    }
    pub fn trigger(&mut self, id: usize, velocity: f32, kit: &DrumKit) {
        if id == 0 || id >= 12 || kit.voices.len() != 12 || !velocity.is_finite() {
            return;
        }
        let d = &kit.voices[id];
        let sr = self.sr;
        let gain = velocity.clamp(0., 3.);
        let tune = 2.0_f32.powf(d.tune.clamp(-24., 24.) / 12.);
        // The 808 and 909 choose opposite priorities when hats are simultaneous.
        if self.kit == 0 {
            if id == 11 {
                self.voices[10].body_fall = decay(0.004, sr);
                self.voices[10].noise_fall = decay(0.004, sr);
                self.voices[10].fall = decay(0.004, sr);
                self.closed_now = true;
            }
        } else {
            if id == 8 {
                self.voices[9] = Voice::default();
            }
            if id == 9 {
                self.voices[8] = Voice::default();
            }
        }
        let old = self.voices[id];
        let mut v = Voice {
            env: 1.,
            body: 1.,
            noise: 1.,
            velocity: gain,
            variant: d.variant,
            band: Band::new(
                if id == 8 && self.kit == 0 {
                    750.
                } else {
                    1800.
                },
                0.9,
                sr,
            ),
            ..Voice::default()
        };
        if self.kit == 0 {
            match id {
                1 => {
                    v.phase[0] = old.phase[0];
                    v.freq[0] = 51.5;
                    v.body = (old.body * 0.35 + 1.).min(1.5);
                    v.body_fall = decay(0.055 + (d.decay.clamp(0.3, 2.) - 0.3) / 1.7 * 0.535, sr);
                    v.pitch = 1.;
                    v.pitch_fall = decay(0.006, sr);
                    v.noise_fall = decay(0.004, sr);
                    v.fall = decay(0.8, sr);
                }
                2 => {
                    v.freq = [178., 332., 0., 0., 0., 0.];
                    let tone = (d.tune / 12. + 0.5).clamp(0., 1.);
                    v.phase[0] = 0.8 - 0.1 * tone;
                    v.phase[1] = 0.505 + 0.11 * tone;
                    // The higher body resonance dies away before the lower one.
                    v.body_fall = decay(0.067, sr);
                    v.pitch = 1.;
                    v.pitch_fall = decay(0.014 + 0.009 * tone, sr);
                    v.noise_fall = decay(0.060, sr);
                    v.fall = v.noise_fall;
                }
                3..=5 => {
                    let j = id - 3;
                    let f = if d.variant {
                        [183., 269., 381.][j]
                    } else {
                        [90., 135., 185.][j]
                    };
                    v.freq[0] = f * tune;
                    v.body_fall = decay(
                        if d.variant {
                            [0.180, 0.082, 0.080][j]
                        } else {
                            [0.200, 0.130, 0.100][j]
                        },
                        sr,
                    );
                    v.noise_fall = decay(0.22, sr);
                    v.pitch = if d.variant { 0.0 } else { 0.28 };
                    v.pitch_fall = decay(0.030, sr);
                    v.fall = v.body_fall;
                }
                6 => {
                    if d.variant {
                        v.freq[0] = 2500.;
                        v.body_fall = decay(0.025, sr)
                    } else {
                        v.freq[0] = 455.;
                        v.freq[1] = 1667.;
                        v.body_fall = decay(0.010, sr)
                    }
                    v.fall = v.body_fall;
                }
                7 => {
                    v.noise_fall = decay(if d.variant { 0.03 } else { 0.10 }, sr);
                    v.fall = v.noise_fall;
                }
                8 => {
                    v.freq[0] = 536.;
                    v.freq[1] = 808.;
                    v.band = Band::new(950., 1.8, sr);
                    v.body_fall = decay(0.35, sr);
                    v.noise_fall = decay(0.035, sr);
                    v.fall = v.body_fall;
                }
                9 => {
                    v.body_fall = decay(0.10 + (d.decay.clamp(0.3, 2.) - 0.3) / 1.7 * 0.37, sr);
                    v.noise_fall = decay(0.046, sr);
                    v.fall = v.body_fall;
                }
                10 => {
                    v.body_fall = decay(
                        if self.closed_now {
                            0.004
                        } else {
                            0.04 + (d.decay.clamp(0.3, 2.) - 0.3) / 1.7 * 0.68
                        },
                        sr,
                    );
                    v.noise_fall = decay(if self.closed_now { 0.004 } else { 0.035 }, sr);
                    v.fall = v.body_fall;
                }
                11 => {
                    v.body_fall = decay(0.030, sr);
                    v.noise_fall = decay(0.012, sr);
                    v.fall = v.body_fall;
                }
                _ => {}
            }
        } else {
            match id {
                1 => {
                    v.freq[0] = 58. * tune;
                    v.body_fall = decay(0.055 + d.decay.clamp(0.3, 2.) * 0.20, sr);
                    v.pitch = 1.1;
                    v.pitch_fall = decay(0.035, sr);
                    v.noise_fall = decay(0.006, sr);
                    v.fall = v.body_fall;
                }
                2 => {
                    v.freq[0] = 172. * tune;
                    v.freq[1] = 238. * tune;
                    v.phase[0] = 0.4;
                    v.phase[1] = 0.72;
                    v.body_fall = decay(0.068 / tune.sqrt(), sr);
                    v.noise_fall = decay(0.035 + 0.13 * d.tone.clamp(0., 1.), sr);
                    v.pitch = 0.1;
                    v.pitch_fall = decay(0.020, sr);
                    v.fall = decay(0.027 / tune, sr);
                }
                3..=5 => {
                    v.freq[0] = [90., 112., 128.][id - 3] * tune;
                    v.freq[1] = [60., 76., 96.][id - 3] * tune;
                    v.body_fall = decay(
                        (0.05 + d.decay.clamp(0.3, 2.) * 0.12) * [1.0, 0.77, 1.0][id - 3],
                        sr,
                    );
                    v.noise_fall = decay(0.022, sr);
                    v.pitch = 0.5;
                    v.pitch_fall = decay(0.025, sr);
                    v.fall = v.body_fall;
                }
                6 => {
                    v.freq[0] = 223.;
                    v.freq[1] = 480.;
                    v.freq[2] = 1037.;
                    v.phase[..3].copy_from_slice(&[0.637, 0.654, 0.494]);
                    v.body_fall = decay(0.042, sr);
                    v.noise_fall = decay(0.030, sr);
                    v.pitch = 1.;
                    v.pitch_fall = decay(0.010, sr);
                    v.fall = v.body_fall;
                }
                7 => {
                    v.noise_fall = decay(0.14, sr);
                    v.body_fall = decay(0.35, sr);
                    v.fall = v.noise_fall;
                }
                8..=11 => {
                    let seconds = match id {
                        8 => 0.025 + d.decay.clamp(0.3, 2.) * 0.035,
                        9 => 0.12 + d.decay.clamp(0.3, 2.) * 0.28,
                        10 => 1.4 / tune,
                        _ => 0.65 / tune,
                    };
                    // Inharmonic resonators and filtered noise; no stored audio or waveform tables.
                    for i in 0..32 {
                        let x = i as f32;
                        let base = match id {
                            8 | 9 => 2300. + x * 260. + (x * x * 19.) % 370.,
                            10 => 460. + x * 295. + (x * x * 31.) % 450.,
                            _ => 650. + x * 550. + (x * x * 23.) % 390.,
                        };
                        let freq = base * if id >= 10 { tune } else { 1. };
                        let weight = if id == 11 {
                            0.08 + (-0.5 * ((freq - 8000.) / 3000.).powi(2)).exp()
                        } else {
                            0.5 + ((i * 17 % 11) as f32) / 20.
                        };
                        let duration = seconds * (0.55 + 0.45 / (1. + x * 0.08));
                        v.modes[i] = Mode {
                            phase: (x * 0.137).fract(),
                            step: if freq < sr * 0.46 { freq / sr } else { 0. },
                            amp: if freq < sr * 0.46 { weight / 20. } else { 0. },
                            fall: decay(duration, sr),
                        };
                    }
                    v.body_fall = decay(seconds, sr);
                    v.noise_fall = decay(if id < 10 { 0.035 } else { 0.11 }, sr);
                    v.fall = v.body_fall;
                }
                _ => {}
            }
        }
        // Shape generated noise to the reference band balance, without storing audio.
        let noise_shape = match (self.kit, id) {
            (0, 2) => Some((3066., 0.737, 1644., 0.349)),
            (0, 9) => Some((6823. * (1. + 0.59 * d.tone), 0.898, 10., 0.172)),
            (0, 10) => Some((8648., 2.689, 6000., 0.001)),
            (1, 2) => Some((7000., 1.15, 10., 0.04)),
            (1, 7) => Some((839., 2.347, 1480., 0.016)),
            (1, 11) => Some((7217., 1.186, 10., 0.409)),
            _ => None,
        };
        if let Some((hz, q, highpass, mix)) = noise_shape {
            v.band = Band::new(hz.min(sr * 0.45), q, sr);
            v.noise_cut = lp(highpass, sr);
            v.noise_mix = mix;
            v.tone_setting = d.tone;
        }
        self.voices[id] = v;
    }
    pub fn sample(&mut self, kit: &DrumKit) -> f32 {
        if kit.voices.len() != 12 {
            return 0.;
        }
        let sr = self.sr;
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        let noise = (self.rng as f32 / u32::MAX as f32) * 2. - 1.;
        let mut metal = 0.;
        for (i, f) in [205.3, 369.6, 304.4, 522.7, 800., 540.].iter().enumerate() {
            metal += square(&mut self.metal[i], *f / sr) / 6.;
        }
        let hi = self.metal_band.process(metal);
        let mut out = 0.;
        for id in 1..12 {
            let d = &kit.voices[id];
            let v = &mut self.voices[id];
            if v.env < 1e-6 && v.body < 1e-6 && v.noise < 1e-6 {
                continue;
            }
            // One shared generated noise source; two one-pole states split high and low bands.
            v.low += self.noise_alpha[0] * (noise - v.low);
            v.high += self.noise_alpha[1] * (noise - v.high);
            let bright = v.high - v.low;
            let air = noise - v.low;
            let sizzle = noise - v.high;
            if self.kit == 0 && id == 9 && d.tone != v.tone_setting {
                let band = Band::new(6823. * (1. + 0.59 * d.tone), 0.898, sr);
                v.band.b = band.b;
                v.band.a1 = band.a1;
                v.band.a2 = band.a2;
                v.tone_setting = d.tone;
            }
            let texture = if v.noise_cut > 0. {
                v.noise_low += v.noise_cut * (noise - v.noise_low);
                let ring = if self.kit == 0 && (id == 9 || id == 10) {
                    metal * 0.8
                } else {
                    0.
                };
                v.band.process(noise - v.noise_low + ring) + sizzle * v.noise_mix
            } else {
                bright
            };
            let t = v.age as f32 / sr;
            let wave = |v: &mut Voice, i: usize, ratio: f32| {
                v.phase[i] = (v.phase[i] + v.freq[i] * ratio / sr).fract();
                (TAU * v.phase[i]).sin()
            };
            let sound = if self.kit == 0 {
                match id {
                    1 => {
                        let f = 1. + v.pitch + 0.06 * v.body;
                        let body = wave(v, 0, f) * v.body;
                        let click = (noise - v.high) * v.noise;
                        let tone = d.tone.clamp(0., 1.);
                        v.tone_state +=
                            self.tone_alpha[(tone * 64.) as usize] * (body - v.tone_state);
                        v.tone_state * 0.8 + click * (0.04 + tone * 0.25)
                    }
                    2 => {
                        let a = wave(v, 0, 1.);
                        let b = wave(v, 1, 1.);
                        // The 808 Tone control blends fixed body resonances rather than transposing them.
                        let tone = (d.tune / 12. + 0.5).clamp(0., 1.);
                        let snappy = d.snappy.clamp(0., 1.);
                        a * (0.9 - 1.05 * tone + 0.4 * tone * tone) * v.body
                            + b * (0.85 + 0.8 * tone) * v.pitch
                            + texture * v.noise * snappy * (2.5 + 3. * snappy)
                    }
                    3..=5 => {
                        let p = 1. + v.pitch;
                        let x = wave(v, 0, p) * v.body;
                        let tail = if id == 3 && !v.variant {
                            v.low * v.noise * 0.15
                        } else {
                            0.
                        };
                        x + tail
                    }
                    6 => {
                        let a = wave(v, 0, 1.);
                        if v.variant {
                            a * v.body
                        } else {
                            let b = wave(v, 1, 1.);
                            (a * 0.55 + b * 0.45) * v.body
                        }
                    }
                    7 => {
                        if v.variant {
                            air * v.noise
                        } else {
                            let burst = if t < 0.010
                                || (t >= 0.014 && t < 0.024)
                                || (t >= 0.028 && t < 0.038)
                            {
                                1.
                            } else {
                                0.
                            };
                            v.band.process(bright) * (burst * 0.8 + v.noise * 0.35)
                        }
                    }
                    8 => {
                        let a = wave(v, 0, 1.).max(0.);
                        let b = wave(v, 1, 1.).max(0.);
                        v.band.process(a * 0.6 + b * 0.8) * v.body * (0.25 + 0.75 * v.noise)
                    }
                    9 => texture * (0.4 * v.body + 0.6 * v.noise),
                    10 => texture * v.body,
                    11 => (hi * 0.35 + sizzle * 0.6) * v.body + air * v.noise * 0.25,
                    _ => 0.,
                }
            } else {
                match id {
                    1 => {
                        let p = 1. + v.pitch;
                        let body = wave(v, 0, p) * v.body;
                        body + bright * v.noise * d.attack.clamp(0., 1.) * 0.45
                    }
                    2 => {
                        let p = 1. + v.pitch;
                        let a = wave(v, 0, p);
                        let b = wave(v, 1, p);
                        let snappy = d.snappy.clamp(0., 1.);
                        let body = (a + 0.8 * (a * a - 0.5)) * 0.5 * v.body
                            + (b + 0.5 * (b * b - 0.5)) * 0.85 * v.env;
                        body * (1. + 1.2 * d.tone.clamp(0., 1.))
                            + texture * v.noise * snappy * (2.2 - 0.4 * snappy)
                    }
                    3..=5 => {
                        let p = 1. + v.pitch;
                        (wave(v, 0, p) * 0.85 + wave(v, 1, 1.) * 0.2) * v.body
                            + bright * v.noise * 0.18
                    }
                    6 => {
                        let a = wave(v, 0, 1.);
                        let b = wave(v, 1, 1.);
                        let c = wave(v, 2, 1.);
                        a * 0.43 * v.body + b * 0.39 * v.noise + c * v.pitch
                    }
                    7 => {
                        let burst = if t < 0.008 {
                            0.4
                        } else if (0.014..0.022).contains(&t) {
                            0.65
                        } else if (0.030..0.045).contains(&t) {
                            1.0
                        } else {
                            0.
                        };
                        texture * (burst + v.noise * 0.35 + v.body * 0.1)
                    }
                    8..=11 => {
                        let mut modal = 0.;
                        for mode in &mut v.modes {
                            mode.phase = (mode.phase + mode.step).fract();
                            modal += (TAU * mode.phase).sin() * mode.amp;
                            mode.amp *= mode.fall;
                        }
                        if id == 11 {
                            modal * 0.65 + texture * (v.body * 0.3 + v.noise * 0.15)
                        } else {
                            modal * 0.65
                                + bright
                                    * v.noise
                                    * if id < 10 {
                                        0.8
                                    } else if id == 10 {
                                        0.6
                                    } else {
                                        0.24
                                    }
                        }
                    }
                    _ => 0.,
                }
            };
            // Calibrated voice balance applies to synthesis, never to the reference recordings.
            let balance = if self.kit == 0 {
                [
                    1., 1.55, 2.12, 1.55, 1.11, 1.56, 1.14, 1.44, 1.8, 0.775, 0.864, 1.05,
                ][id]
            } else {
                [
                    1., 0.668, 1.96, 1.79, 1.38, 1.73, 2.5, 6.83, 2.32, 3.02, 2.04, 3.02,
                ][id]
            };
            out += sound * v.velocity * d.level.clamp(0., 1.5) * 0.19 * balance;
            v.env *= v.fall;
            v.body *= v.body_fall;
            v.noise *= v.noise_fall;
            v.pitch *= v.pitch_fall;
            v.age = v.age.saturating_add(1);
        }
        self.closed_now = false;
        out
    }
}
// time to one tenth peak, as used in the 808 service notes.
fn decay(seconds: f32, sr: f32) -> f32 {
    (-std::f32::consts::LN_10 / (seconds.max(0.001) * sr)).exp()
}
fn lp(hz: f32, sr: f32) -> f32 {
    1. - (-TAU * hz.min(sr * 0.45) / sr).exp()
}
fn square(phase: &mut f32, dt: f32) -> f32 {
    let dt = dt.clamp(0., 0.45);
    let p = *phase;
    *phase = (p + dt).fract();
    let mut y = if p < 0.5 { 1. } else { -1. };
    y += blep(p, dt);
    y -= blep((p + 0.5).fract(), dt);
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
            if k == 1 {
                assert!(d.voices[oh].body == 0.);
            }
            d.trigger(oh, 1., &kit);
            if k == 0 {
                assert!(d.voices[oh].body_fall < decay(0.01, 48000.))
            } else {
                assert!(d.voices[ch].body == 0. && d.voices[oh].body > 0.)
            }
        }
    }
    #[test]
    fn tuning_and_decay_anchors() {
        let kit = Song::default().drums[0].clone();
        let mut d = DrumSynth::new(48000., 0);
        d.trigger(2, 1., &kit);
        assert_eq!(d.voices[2].freq[0], 178.);
        assert_eq!(d.voices[2].freq[1], 332.);
        assert!((d.voices[2].body_fall.powf(48000. * 0.067) - 0.1).abs() < 0.001);
        d.trigger(8, 1., &kit);
        assert_eq!(d.voices[8].freq[0], 536.);
        assert_eq!(d.voices[8].freq[1], 808.);
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
